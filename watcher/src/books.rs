//! The ledger replayed by the watcher (spec §19): from the spark log of each epoch and the wishes
//! the server lists, it checks every spark, adds the work, selects the miracles with the same
//! rules as the server, follows what the world did with them, and must arrive at the ledger root
//! the operator signed. It starts at the first signed header (there is no signed ledger before).

use std::collections::{BTreeMap, HashMap};

use protogaea_core::miracle::{check_wish, is_transient, price_share, Outcomes};
use protogaea_core::run::Run;
use protogaea_core::Miracle;
use protogaea_pow::{meets, spark_input, weight, Hasher, SPARK};
use protogaea_protocol::header::{ledger_leaf, ledger_root};
use protogaea_protocol::ledger::{select, Candidate};
use protogaea_protocol::log::{leaf_hash, root};
use protogaea_protocol::spark::{challenge, next_target, Spark};
use protogaea_protocol::wish::{Action, CladeAction, Source, Weather, Wish};
use protogaea_protocol::{hex, Hash};
use serde_json::{json, Value};

pub type Alarm = (&'static str, Value);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Status {
    Open,
    Ready,
    Selected,
    Executed,
    Expired,
    Invalidated,
}

struct Entry {
    wish: Wish,
    work: u128,
    status: Status,
}

pub struct Books {
    pub active: bool,
    price: u128,
    price_min: u128,
    wishes: BTreeMap<Hash, Entry>,
    /// The last epoch's window: its epoch, target and the sparks it accepted.
    last_window: Option<(u64, u64, u64)>,
    threads: usize,
}

/// A wish's miracle for the core, as the server makes it: cells as `x + y · width`.
pub fn to_miracle(w: &Wish, width: u16) -> Miracle {
    let cell = |(x, y): (u8, u8)| u16::from(x) + u16::from(y) * width;
    match &w.action {
        Action::Weather { x, y, kind } => Miracle::Weather {
            center: cell((*x, *y)),
            rain: *kind == Weather::Rain,
        },
        Action::Migrate { clade_id, from, to } => Miracle::Migrate {
            clade_id: *clade_id,
            from: cell(*from),
            to: cell(*to),
        },
        Action::Revive {
            source,
            entry_id,
            steps,
            at,
        } => Miracle::Revive {
            from_museum: *source == Source::Museum,
            entry_id: *entry_id,
            steps: steps.clone(),
            at: cell(*at),
        },
        Action::Clade {
            action,
            clade_id,
            at,
        } => Miracle::Clade {
            action: clade_action(*action),
            clade_id: *clade_id,
            center: cell(*at),
        },
        Action::Hybrid {
            clade_a,
            clade_b,
            at,
        } => Miracle::Hybrid {
            clade_a: *clade_a,
            clade_b: *clade_b,
            center: cell(*at),
        },
    }
}

/// A patron's action as the core codes it (`miracle::SHELTER` … and `GIFT + k`).
pub fn clade_action(a: CladeAction) -> u8 {
    use protogaea_core::miracle as m;
    match a {
        CladeAction::Shelter => m::SHELTER,
        CladeAction::Forage => m::FORAGE,
        CladeAction::Cure => m::CURE,
        CladeAction::Gift(g) => m::GIFT + g,
        CladeAction::Blight => m::BLIGHT,
        CladeAction::Expose => m::EXPOSE,
        CladeAction::Sickness => m::SICKNESS,
    }
}

fn unhex_vec(s: &str) -> Option<Vec<u8>> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok())
        .collect()
}

fn h32(v: &Value) -> Option<Hash> {
    unhex_vec(v.as_str()?)?.try_into().ok()
}

impl Books {
    pub fn new(price_min: u128, threads: usize) -> Self {
        Self {
            active: false,
            price: price_min,
            price_min,
            wishes: BTreeMap::new(),
            last_window: None,
            threads: threads.max(1),
        }
    }

    /// Checks every spark's PoW on several threads; returns the indexes that fail.
    fn bad_pow(
        &self,
        sparks: &[Spark],
        world_id: &[u8; 16],
        epoch: u64,
        ch: &Hash,
        target: u64,
    ) -> Vec<usize> {
        let per = sparks.len().div_ceil(self.threads).max(1);
        std::thread::scope(|s| {
            let handles: Vec<_> = sparks
                .chunks(per)
                .enumerate()
                .map(|(k, chunk)| {
                    s.spawn(move || {
                        let mut h = Hasher::new(SPARK);
                        chunk
                            .iter()
                            .enumerate()
                            .filter(|(_, sp)| {
                                !meets(
                                    &h.hash(&spark_input(
                                        world_id,
                                        epoch,
                                        ch,
                                        &sp.proposal_id,
                                        &sp.miner,
                                        sp.nonce,
                                    )),
                                    target,
                                )
                            })
                            .map(|(i, _)| k * per + i)
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("a checking thread"))
                .collect()
        })
    }

    /// The window of epoch `e`, closed before the world steps into it: sparks checked against
    /// the header, work added, open wishes expired, and the miracles selected. Returns the ids
    /// selected, in order.
    pub fn close(
        &mut self,
        e: u64,
        header: &Value,
        log: &Value,
        known: &HashMap<Hash, Wish>,
        run: &Run,
        alarms: &mut Vec<Alarm>,
    ) -> Vec<Hash> {
        let world_id = &run.world.world_id;
        let prev = h32(&header["prev_header_hash"]).unwrap_or([0; 32]);
        let ch = h32(&log["challenge"]).unwrap_or([0; 32]);
        if prev != [0; 32] && ch != challenge(world_id, e, &prev) {
            alarms.push((
                "a window's challenge does not commit to the previous header",
                json!({ "epoch": e }),
            ));
        }
        let target: u64 = log["target"]
            .as_str()
            .and_then(|t| t.parse().ok())
            .unwrap_or(0);
        if let Some((pe, pt, pa)) = self.last_window {
            if pe + 1 == e && next_target(pt, pa) != target {
                alarms.push(("the target did not follow its rule", json!({ "epoch": e, "target": target.to_string(), "expected": next_target(pt, pa).to_string() })));
            }
        }
        let sparks: Vec<Spark> = log["sparks"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| {
                unhex_vec(s.as_str()?)?
                    .try_into()
                    .ok()
                    .map(|b: [u8; 72]| Spark::from_bytes(&b))
            })
            .collect();
        self.last_window = Some((e, target, sparks.len() as u64));
        if log["accepted"]
            .as_u64()
            .is_some_and(|a| a != sparks.len() as u64)
        {
            alarms.push((
                "the window's count differs from its log",
                json!({ "epoch": e }),
            ));
        }
        // The log the header commits to.
        let leaves: Vec<Hash> = sparks.iter().map(|s| leaf_hash(&s.leaf(e))).collect();
        if header["sth_size"].as_u64() != Some(sparks.len() as u64)
            || h32(&header["sth_root"]) != Some(root(&leaves))
        {
            alarms.push((
                "the epoch's log is not the one its header commits to",
                json!({ "epoch": e, "sparks": sparks.len() }),
            ));
        }
        let bad = self.bad_pow(&sparks, world_id, e, &ch, target);
        if !bad.is_empty() {
            alarms.push((
                "sparks in the log do not meet their target",
                json!({ "epoch": e, "count": bad.len() }),
            ));
        }
        // Work: a wish enters the ledger with its first spark.
        let w = u128::from(weight(target.max(1)));
        for (i, s) in sparks.iter().enumerate() {
            if bad.contains(&i) {
                continue;
            }
            match self.wishes.entry(s.proposal_id) {
                std::collections::btree_map::Entry::Occupied(mut o) => o.get_mut().work += w,
                std::collections::btree_map::Entry::Vacant(v) => match known.get(&s.proposal_id) {
                    Some(wish) => {
                        v.insert(Entry {
                            wish: wish.clone(),
                            work: w,
                            status: Status::Open,
                        });
                    }
                    None => alarms.push((
                        "a spark for a wish nobody knows",
                        json!({ "epoch": e, "wish": hex(&s.proposal_id) }),
                    )),
                },
            }
        }
        for entry in self.wishes.values_mut() {
            if entry.status == Status::Open && entry.wish.expires_epoch <= e {
                entry.status = Status::Expired;
            }
        }
        let candidates: Vec<Candidate> = self
            .wishes
            .iter()
            .filter(|(_, x)| matches!(x.status, Status::Open | Status::Ready))
            .filter_map(|(id, x)| {
                let m = to_miracle(&x.wish, run.world.width);
                Some(Candidate {
                    id: *id,
                    action: x.wish.action.clone(),
                    work: x.work,
                    share: u128::from(price_share(&run.world, &run.rules, &m)?),
                })
            })
            .collect();
        let beacon = h32(&header["beacon"]).unwrap_or([0; 32]);
        let s = select(&candidates, self.price, self.price_min, &beacon);
        for id in &s.ready {
            self.wishes.get_mut(id).expect("a candidate").status = Status::Ready;
        }
        for id in &s.selected {
            self.wishes.get_mut(id).expect("a candidate").status = Status::Selected;
        }
        self.price = s.next_price;
        s.selected
    }

    /// After the world stepped: what it did with the selected miracles, the ledger root the
    /// header signed, then the soft check of the next window.
    pub fn settle(
        &mut self,
        e: u64,
        selected: &[Hash],
        outcomes: &Outcomes,
        run: &Run,
        header: &Value,
        alarms: &mut Vec<Alarm>,
    ) {
        for (i, id) in selected.iter().enumerate() {
            let refused = outcomes
                .refused
                .iter()
                .find(|(k, _)| *k == i)
                .map(|(_, why)| *why);
            self.wishes.get_mut(id).expect("selected").status = match refused {
                None => Status::Executed,
                Some(why) if is_transient(why) => Status::Ready,
                Some(_) => Status::Invalidated,
            };
        }
        let leaves: Vec<Hash> = self
            .wishes
            .iter()
            .filter(|(_, x)| matches!(x.status, Status::Open | Status::Ready))
            .map(|(id, x)| ledger_leaf(id, x.work, x.status == Status::Ready))
            .collect();
        if h32(&header["ledger_root"]) != Some(ledger_root(self.price, &leaves)) {
            alarms.push((
                "the ledger does not replay to the signed ledger root",
                json!({ "epoch": e, "price": self.price.to_string(), "wishes": leaves.len() }),
            ));
        }
        for x in self.wishes.values_mut() {
            if matches!(x.status, Status::Open | Status::Ready) {
                if let Err(why) = check_wish(
                    &run.world,
                    &run.rules,
                    &to_miracle(&x.wish, run.world.width),
                ) {
                    if !is_transient(why) {
                        x.status = Status::Invalidated;
                    }
                }
            }
        }
    }
}
