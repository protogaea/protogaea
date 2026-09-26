//! Wishes and sparks (stage C, spec §17–§18): the open window, intake, the spark log, signed tree
//! heads and receipts, and the work each wish accumulates when a window closes.
//!
//! While the world shows epoch e, the window of epoch E = e + 1 is open: its challenge commits to
//! the header of e. When the timer fires, the window closes with a final signed tree head, the
//! work of the epoch's sparks is added to their wishes and the target moves; then the world steps
//! to E and the window of E + 1 opens.
//!
//! The spark log lives in `sparks.sqlite`, apart from the world's database: the world may roll
//! back to its last snapshot after a crash, the log never does (the miracles applied past the
//! snapshot are rolled back with the world).
//!
//! The ledger (spec §19) runs when a window closes: wishes whose work covers their price are
//! ready; the ready ones are ranked by the share of the price they cover and up to three that do
//! not conflict are selected; the price then moves by an eighth. The world applies the selected
//! miracles as it steps; one refused for a reason that passes by itself (an active effect, a
//! cooldown) goes back to the queue, one refused for good is invalidated.

use std::collections::HashSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use protogaea_core::miracle::{check, is_transient, Outcomes};
use protogaea_core::{Miracle, Ruleset, World};
use protogaea_protocol::log::{consistency_proof, inclusion_proof, leaf_hash, root};
use protogaea_protocol::spark::{challenge, next_target, Spark};
use protogaea_protocol::sth::{Receipt, Sth};
use protogaea_protocol::wish::{self, Action, Source, Weather, Wish, MAX_LIFETIME};
use protogaea_protocol::{hex, Hash};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

pub const DB: &str = "sparks.sqlite";
const KEY: &str = "operator.key";
/// The first target: a spark is worth 256 hashes (2⁵⁶ of 2⁶⁴).
const FIRST_TARGET: u64 = 1 << 56;
/// Open wishes per author key at most (spec §17).
pub const OPEN_PER_AUTHOR: i64 = 3;
/// Miracles per epoch at most, and each action's price multiplier in percent (spec §19).
pub const PER_EPOCH: usize = 3;
pub const PRICE_MULT: [u128; 3] = [100, 120, 200];
pub const TIEBREAK_TAG: &[u8] = b"PROTOGAEA/TIEBREAK/V0";

/// A wish's miracle for the core: cells as `x + y · width`.
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
    }
}

/// Two miracles that cannot go in one epoch (spec §5): overlapping weather areas, one clade
/// relocated twice, or targets whose 3 × 3 areas overlap.
fn conflicts(a: &Action, b: &Action) -> bool {
    let far = |p: (u8, u8), q: (u8, u8)| {
        (i32::from(p.0) - i32::from(q.0))
            .abs()
            .max((i32::from(p.1) - i32::from(q.1)).abs())
    };
    let target = |a: &Action| match a {
        Action::Migrate { to, .. } => Some(*to),
        Action::Revive { at, .. } => Some(*at),
        Action::Weather { .. } => None,
    };
    match (a, b) {
        (Action::Weather { x, y, .. }, Action::Weather { x: x2, y: y2, .. }) => {
            far((*x, *y), (*x2, *y2)) <= 6
        }
        (Action::Migrate { clade_id: c1, .. }, Action::Migrate { clade_id: c2, .. })
            if c1 == c2 =>
        {
            true
        }
        _ => matches!((target(a), target(b)), (Some(p), Some(q)) if far(p, q) <= 2),
    }
}

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Why a wish or a spark was refused (protocol §10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Format,
    WindowClosed,
    UnknownProposal,
    ProposalClosed,
    Duplicate,
    PowInvalid,
    Signature,
    ActionInvalid(&'static str),
    Limit,
}

impl Refusal {
    pub fn code(&self) -> &'static str {
        match self {
            Refusal::Format => "E_FORMAT",
            Refusal::WindowClosed => "E_WINDOW_CLOSED",
            Refusal::UnknownProposal => "E_UNKNOWN_PROPOSAL",
            Refusal::ProposalClosed => "E_PROPOSAL_CLOSED",
            Refusal::Duplicate => "E_DUPLICATE",
            Refusal::PowInvalid => "E_POW_INVALID",
            Refusal::Signature => "E_SIGNATURE",
            Refusal::ActionInvalid(_) => "E_ACTION_INVALID",
            Refusal::Limit => "E_RATE_LIMIT",
        }
    }
}

/// What a spark needs checked with the PoW, taken under the lock and checked outside it.
#[derive(Clone, Copy)]
pub struct Ticket {
    pub epoch: u64,
    pub challenge: Hash,
    pub target: u64,
    pub world_id: [u8; 16],
}

pub struct Intake {
    conn: Connection,
    secret: [u8; 32],
    pub operator: [u8; 32],
    pub world_id: [u8; 16],
    pub ruleset_id: Hash,
    /// The epoch whose window is open, its challenge and target.
    pub epoch: u64,
    pub challenge: Hash,
    pub target: u64,
    pub open: bool,
    /// The leaf hashes of this epoch's log, and the spark ids in it.
    leaves: Vec<Hash>,
    seen: HashSet<Hash>,
    pub sth: Option<Sth>,
    /// The window before the open one: a spark that fails the open window's PoW but passes this
    /// one was only late (the client had not seen the change yet), not forged.
    pub previous: Option<Ticket>,
    /// The floor of the price of a miracle, in work units (spec §19, `P_min`).
    pub price_min: u128,
}

/// Reads the operator's secret key from the data directory, or makes one.
fn operator_key(data: &Path) -> Result<[u8; 32], String> {
    let path = data.join(KEY);
    match std::fs::read(&path) {
        Ok(bytes) => bytes
            .try_into()
            .map_err(|_| format!("{} is not a 32-byte key", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut key = [0u8; 32];
            getrandom::fill(&mut key).map_err(|e| e.to_string())?;
            std::fs::write(&path, key)
                .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
            println!("made a new operator key in {}", path.display());
            Ok(key)
        }
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

impl Intake {
    pub fn open(
        data: &Path,
        world_id: [u8; 16],
        ruleset_id: Hash,
        price_min: u128,
    ) -> Result<Intake, String> {
        let secret = operator_key(data)?;
        let conn = Connection::open(data.join(DB)).map_err(err)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(err)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS wishes (
                 id BLOB PRIMARY KEY,
                 bytes BLOB NOT NULL,
                 signature BLOB NOT NULL,
                 author BLOB NOT NULL,
                 action INTEGER NOT NULL,
                 created_epoch INTEGER NOT NULL,
                 expires_epoch INTEGER NOT NULL,
                 status TEXT NOT NULL,
                 work TEXT NOT NULL,
                 at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS wishes_by_author ON wishes (author, status);
             CREATE TABLE IF NOT EXISTS sparks (
                 epoch INTEGER NOT NULL,
                 idx INTEGER NOT NULL,
                 id BLOB NOT NULL UNIQUE,
                 proposal_id BLOB NOT NULL,
                 miner BLOB NOT NULL,
                 nonce INTEGER NOT NULL,
                 weight INTEGER NOT NULL,
                 PRIMARY KEY (epoch, idx)
             );
             CREATE TABLE IF NOT EXISTS sths (
                 epoch INTEGER NOT NULL,
                 tree_size INTEGER NOT NULL,
                 root BLOB NOT NULL,
                 at INTEGER NOT NULL,
                 signature BLOB NOT NULL,
                 final INTEGER NOT NULL,
                 PRIMARY KEY (epoch, tree_size)
             );
             CREATE TABLE IF NOT EXISTS windows (
                 epoch INTEGER PRIMARY KEY,
                 challenge BLOB NOT NULL,
                 target INTEGER NOT NULL,
                 accepted INTEGER
             );
             CREATE TABLE IF NOT EXISTS miracles (
                 epoch INTEGER NOT NULL,
                 idx INTEGER NOT NULL,
                 proposal_id BLOB NOT NULL,
                 miracle TEXT NOT NULL,
                 outcome TEXT NOT NULL,
                 PRIMARY KEY (epoch, idx)
             );
             CREATE TABLE IF NOT EXISTS ledger (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .map_err(err)?;
        // Columns added with the ledger, for a log made before it.
        for column in ["reason TEXT", "executed_epoch INTEGER"] {
            let _ = conn.execute(&format!("ALTER TABLE wishes ADD COLUMN {column}"), []);
        }
        let operator = wish::public_key(&secret);
        Ok(Intake {
            conn,
            secret,
            operator,
            world_id,
            ruleset_id,
            epoch: 0,
            challenge: [0; 32],
            target: FIRST_TARGET,
            open: false,
            leaves: Vec::new(),
            seen: HashSet::new(),
            sth: None,
            previous: None,
            price_min,
        })
    }

    /// The price of a miracle for the next selection (`P_E`), in work units.
    pub fn price(&self) -> Result<u128, String> {
        let v: Option<String> = self
            .conn
            .query_row("SELECT value FROM ledger WHERE key = 'price'", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(err)?;
        Ok(v.and_then(|v| v.parse().ok())
            .unwrap_or(self.price_min)
            .max(self.price_min))
    }

    fn set_price(&self, p: u128) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO ledger (key, value) VALUES ('price', ?1)",
                [p.to_string()],
            )
            .map_err(err)?;
        Ok(())
    }

    /// After a restart from a snapshot at `epoch`: the miracles of later epochs are undone with
    /// the world, and their wishes go back to the queue.
    pub fn rollback_after(&mut self, epoch: u64) -> Result<(), String> {
        let e = epoch as i64;
        self.conn
            .execute_batch(&format!(
                "DELETE FROM miracles WHERE epoch > {e};
                 UPDATE wishes SET status = 'ready', executed_epoch = NULL, reason = NULL
                   WHERE executed_epoch > {e} OR status = 'selected';"
            ))
            .map_err(err)
    }

    /// The soft check when a window opens (spec §5): open and queued wishes that can no longer
    /// apply, for a reason that will not pass, are invalidated; their work is burned.
    pub fn soft_check(&mut self, world: &World, rules: &Ruleset) -> Result<u32, String> {
        let rows: Vec<(Vec<u8>, Vec<u8>)> = {
            let mut stmt = self
                .conn
                .prepare("SELECT id, bytes FROM wishes WHERE status IN ('open', 'ready')")
                .map_err(err)?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(err)?
                .collect::<Result<_, _>>()
                .map_err(err)?;
            rows
        };
        let mut closed = 0;
        for (id, bytes) in rows {
            let Ok(w) = Wish::from_bytes(&bytes) else {
                continue;
            };
            if let Err(why) = check(world, rules, &to_miracle(&w, world.width)) {
                if !is_transient(why) {
                    self.conn
                        .execute(
                            "UPDATE wishes SET status = 'invalidated', reason = ?2 WHERE id = ?1",
                            params![id, why],
                        )
                        .map_err(err)?;
                    closed += 1;
                }
            }
        }
        Ok(closed)
    }

    /// The ledger (spec §19), after the window's work was added: ready wishes, ranked, up to
    /// three selected without conflicts, and the next price. Returns the selected wishes.
    fn select(&mut self, beacon: &Hash) -> Result<Vec<(Hash, Wish)>, String> {
        let price = self.price()?;
        let rows: Vec<(Vec<u8>, Vec<u8>, String)> = {
            let mut stmt = self
                .conn
                .prepare("SELECT id, bytes, work FROM wishes WHERE status IN ('open', 'ready')")
                .map_err(err)?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(err)?
                .collect::<Result<_, _>>()
                .map_err(err)?;
            rows
        };
        let mut ready: Vec<(Hash, Wish, u128, u128)> = Vec::new();
        for (id, bytes, work) in rows {
            let Ok(w) = Wish::from_bytes(&bytes) else {
                continue;
            };
            let mult = PRICE_MULT[usize::from(w.action.code())];
            let work: u128 = work.parse().unwrap_or(0);
            if work >= price * mult / 100 {
                self.conn
                    .execute("UPDATE wishes SET status = 'ready' WHERE id = ?1", [&id])
                    .map_err(err)?;
                ready.push((blob32(id), w, work, mult));
            }
        }
        // By the share of the price covered, W / mult, compared by cross-multiplication; ties by
        // BLAKE3(tag ‖ beacon ‖ proposal_id).
        ready.sort_by(|a, b| {
            (b.2 * a.3).cmp(&(a.2 * b.3)).then_with(|| {
                protogaea_protocol::hash(&[TIEBREAK_TAG, beacon, &a.0])
                    .cmp(&protogaea_protocol::hash(&[TIEBREAK_TAG, beacon, &b.0]))
            })
        });
        let mut selected: Vec<(Hash, Wish)> = Vec::new();
        for (id, w, _, _) in &ready {
            if selected.len() == PER_EPOCH {
                break;
            }
            if selected
                .iter()
                .any(|(_, s)| conflicts(&s.action, &w.action))
            {
                continue;
            }
            selected.push((*id, w.clone()));
        }
        for (id, _) in &selected {
            self.conn
                .execute(
                    "UPDATE wishes SET status = 'selected' WHERE id = ?1",
                    [id.to_vec()],
                )
                .map_err(err)?;
        }
        let next = if ready.len() > selected.len() {
            price + price / 8
        } else if selected.len() < PER_EPOCH {
            (price - price / 8).max(self.price_min)
        } else {
            price
        };
        self.set_price(next)?;
        Ok(selected)
    }

    /// What the world did with the selected miracles of `epoch`: applied ones are executed,
    /// refused ones wait in the queue or are invalidated. Every miracle given is logged, so that
    /// anyone can replay the epoch.
    pub fn record(
        &mut self,
        epoch: u64,
        selected: &[(Hash, Wish)],
        miracles: &[Miracle],
        outcomes: &Outcomes,
    ) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(err)?;
        for (i, ((id, _), m)) in selected.iter().zip(miracles).enumerate() {
            let refused = outcomes
                .refused
                .iter()
                .find(|(k, _)| *k == i)
                .map(|(_, why)| *why);
            let (status, outcome) = match refused {
                None => ("executed", "applied".to_string()),
                Some(why) if is_transient(why) => ("ready", format!("deferred: {why}")),
                Some(why) => ("invalidated", format!("refused: {why}")),
            };
            tx.execute(
                "UPDATE wishes SET status = ?2, reason = ?3, executed_epoch = ?4 WHERE id = ?1",
                params![
                    id.to_vec(),
                    status,
                    refused,
                    (status == "executed").then_some(epoch as i64)
                ],
            )
            .map_err(err)?;
            tx.execute(
                "INSERT OR REPLACE INTO miracles (epoch, idx, proposal_id, miracle, outcome) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    epoch as i64,
                    i as i64,
                    id.to_vec(),
                    serde_json::to_string(m).map_err(|e| e.to_string())?,
                    outcome
                ],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)
    }

    /// The miracles given to the world for epochs `from` to `to`, in order, with their outcomes.
    pub fn miracles(&self, from: u64, to: u64) -> Result<Vec<Value>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT epoch, idx, proposal_id, miracle, outcome FROM miracles
                 WHERE epoch BETWEEN ?1 AND ?2 ORDER BY epoch, idx LIMIT 5000",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map(params![from as i64, to as i64], |r| {
                Ok(json!({
                    "epoch": r.get::<_, i64>(0)?,
                    "proposal_id": hex(&r.get::<_, Vec<u8>>(2)?),
                    "miracle": serde_json::from_str::<Value>(&r.get::<_, String>(3)?).unwrap_or(Value::Null),
                    "outcome": r.get::<_, String>(4)?,
                }))
            })
            .map_err(err)?;
        rows.map(|r| r.map_err(err)).collect()
    }

    /// Opens the window of `epoch`, whose challenge commits to the previous header (`prev_root`
    /// stands in for its hash until headers are signed). After a restart the window picks up the
    /// sparks it already has.
    pub fn open_window(&mut self, epoch: u64, prev_root: &Hash) -> Result<(), String> {
        let known: Option<(Vec<u8>, i64)> = self
            .conn
            .query_row(
                "SELECT challenge, target FROM windows WHERE epoch = ?1",
                [epoch as i64],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(err)?;
        let (challenge, target) = match known {
            Some((c, t)) => (c.try_into().map_err(|_| "a bad challenge")?, t as u64),
            None => {
                // The target follows the last closed window.
                let last: Option<(i64, Option<i64>)> = self
                    .conn
                    .query_row(
                        "SELECT target, accepted FROM windows WHERE epoch < ?1 ORDER BY epoch DESC LIMIT 1",
                        [epoch as i64],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()
                    .map_err(err)?;
                let target = match last {
                    Some((t, Some(accepted))) => next_target(t as u64, accepted as u64),
                    Some((t, None)) => t as u64,
                    None => FIRST_TARGET,
                };
                let c = challenge(&self.world_id, epoch, prev_root);
                self.conn
                    .execute(
                        "INSERT INTO windows (epoch, challenge, target) VALUES (?1, ?2, ?3)",
                        params![epoch as i64, c.to_vec(), target.min(i64::MAX as u64) as i64],
                    )
                    .map_err(err)?;
                (c, target)
            }
        };
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, proposal_id, miner, nonce FROM sparks WHERE epoch = ?1 ORDER BY idx",
            )
            .map_err(err)?;
        let rows: Vec<(Vec<u8>, Spark)> = stmt
            .query_map([epoch as i64], |r| {
                Ok((
                    r.get::<_, Vec<u8>>(0)?,
                    Spark {
                        proposal_id: blob32(r.get(1)?),
                        miner: blob32(r.get(2)?),
                        nonce: r.get::<_, i64>(3)? as u64,
                    },
                ))
            })
            .map_err(err)?
            .collect::<Result<_, _>>()
            .map_err(err)?;
        drop(stmt);
        if self.challenge != [0; 32] && self.epoch + 1 == epoch {
            self.previous = Some(Ticket {
                epoch: self.epoch,
                challenge: self.challenge,
                target: self.target,
                world_id: self.world_id,
            });
        }
        self.epoch = epoch;
        self.challenge = challenge;
        self.target = target;
        self.leaves = rows
            .iter()
            .map(|(_, s)| leaf_hash(&s.leaf(epoch)))
            .collect();
        self.seen = rows.iter().map(|(id, _)| blob32(id.clone())).collect();
        self.open = true;
        self.sign_head(false)?;
        Ok(())
    }

    /// Closes the window: the final signed tree head, the work of its sparks added to their
    /// wishes, open wishes past their lifetime expired (queued ones wait), and the ledger's
    /// selection. Returns the wishes selected for the epoch.
    pub fn close_window(&mut self, beacon: &Hash) -> Result<Vec<(Hash, Wish)>, String> {
        if !self.open {
            return Ok(Vec::new());
        }
        self.open = false;
        let sth = self.sign_head(true)?;
        let epoch = self.epoch as i64;
        let tx = self.conn.transaction().map_err(err)?;
        tx.execute(
            "UPDATE windows SET accepted = ?2 WHERE epoch = ?1",
            params![epoch, self.leaves.len() as i64],
        )
        .map_err(err)?;
        let sums: Vec<(Vec<u8>, i64)> = {
            let mut stmt = tx
                .prepare("SELECT proposal_id, sum(weight) FROM sparks WHERE epoch = ?1 GROUP BY proposal_id")
                .map_err(err)?;
            let rows = stmt
                .query_map([epoch], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(err)?
                .collect::<Result<_, _>>()
                .map_err(err)?;
            rows
        };
        for (id, add) in sums {
            let work: Option<String> = tx
                .query_row("SELECT work FROM wishes WHERE id = ?1", [&id], |r| r.get(0))
                .optional()
                .map_err(err)?;
            let total = work.and_then(|w| w.parse::<u128>().ok()).unwrap_or(0) + add as u128;
            tx.execute(
                "UPDATE wishes SET work = ?2 WHERE id = ?1",
                params![id, total.to_string()],
            )
            .map_err(err)?;
        }
        tx.execute(
            "UPDATE wishes SET status = 'expired' WHERE status = 'open' AND expires_epoch <= ?1",
            [epoch],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        let _ = sth;
        self.select(beacon)
    }

    /// Signs and records a tree head of the current log.
    fn sign_head(&mut self, final_head: bool) -> Result<Sth, String> {
        let sth = Sth::sign(
            &self.secret,
            self.epoch,
            self.leaves.len() as u64,
            root(&self.leaves),
            now_ms(),
        );
        self.conn
            .execute(
                "INSERT OR REPLACE INTO sths (epoch, tree_size, root, at, signature, final)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    sth.epoch as i64,
                    sth.tree_size as i64,
                    sth.root.to_vec(),
                    sth.timestamp_ms as i64,
                    sth.signature.to_vec(),
                    final_head
                ],
            )
            .map_err(err)?;
        self.sth = Some(sth);
        Ok(sth)
    }

    pub fn ticket(&self) -> Result<Ticket, Refusal> {
        if !self.open {
            return Err(Refusal::WindowClosed);
        }
        Ok(Ticket {
            epoch: self.epoch,
            challenge: self.challenge,
            target: self.target,
            world_id: self.world_id,
        })
    }

    /// The checks before the PoW (protocol §5.6, steps 2–4) for one spark.
    pub fn precheck(&self, spark: &Spark) -> Result<(), Refusal> {
        if !self.open {
            return Err(Refusal::WindowClosed);
        }
        let status: Option<String> = self
            .conn
            .query_row(
                "SELECT status FROM wishes WHERE id = ?1",
                [spark.proposal_id.to_vec()],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| Refusal::Format)?;
        match status.as_deref() {
            None => return Err(Refusal::UnknownProposal),
            Some("open") => {}
            Some(_) => return Err(Refusal::ProposalClosed),
        }
        if self.seen.contains(&spark.id(self.epoch)) {
            return Err(Refusal::Duplicate);
        }
        Ok(())
    }

    /// The checks of a new wish that need no PoW (protocol §4.4): format, signature, the world
    /// and ruleset, the epochs, and the author's open wishes.
    pub fn precheck_wish(&self, bytes: &[u8], signature: &[u8; 64]) -> Result<Wish, Refusal> {
        let w = Wish::from_bytes(bytes).map_err(|_| Refusal::Format)?;
        let id = w.id();
        wish::verify(&w.author, &id, signature).map_err(|_| Refusal::Signature)?;
        if w.world_id != self.world_id || w.ruleset_id != self.ruleset_id {
            return Err(Refusal::ActionInvalid("another world or ruleset"));
        }
        if !self.open {
            return Err(Refusal::WindowClosed);
        }
        // Made for the open window (or the one just before), and not outliving the limit.
        if w.created_epoch + 1 < self.epoch || w.created_epoch > self.epoch {
            return Err(Refusal::ActionInvalid(
                "created_epoch is not the open window",
            ));
        }
        if w.expires_epoch > w.created_epoch + MAX_LIFETIME {
            return Err(Refusal::Format);
        }
        let exists: bool = self
            .conn
            .query_row(
                "SELECT count(*) FROM wishes WHERE id = ?1",
                [id.to_vec()],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .map_err(|_| Refusal::Format)?;
        if exists {
            return Err(Refusal::Duplicate);
        }
        let open: i64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM wishes WHERE author = ?1 AND status = 'open'",
                [w.author.to_vec()],
                |r| r.get(0),
            )
            .map_err(|_| Refusal::Format)?;
        if open >= OPEN_PER_AUTHOR {
            return Err(Refusal::Limit);
        }
        Ok(w)
    }

    pub fn add_wish(&mut self, w: &Wish, bytes: &[u8], signature: &[u8; 64]) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO wishes (id, bytes, signature, author, action, created_epoch, expires_epoch, status, work, at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'open', '0', ?8)",
                params![
                    w.id().to_vec(),
                    bytes,
                    signature.to_vec(),
                    w.author.to_vec(),
                    w.action.code(),
                    w.created_epoch as i64,
                    w.expires_epoch as i64,
                    now_ms() as i64
                ],
            )
            .map_err(err)?;
        Ok(())
    }

    /// Appends sparks whose PoW passed for `ticket`; returns a receipt for each, all under one
    /// fresh tree head. A spark whose window has closed meanwhile, or a duplicate, is refused.
    pub fn append(
        &mut self,
        ticket: &Ticket,
        sparks: &[(Spark, u64)],
    ) -> Result<Vec<Result<Receipt, Refusal>>, String> {
        let mut placed: Vec<Result<(Spark, u64), Refusal>> = Vec::new();
        let tx = self.conn.transaction().map_err(err)?;
        for (s, weight) in sparks {
            if !self.open || self.epoch != ticket.epoch {
                placed.push(Err(Refusal::WindowClosed));
                continue;
            }
            let id = s.id(self.epoch);
            if !self.seen.insert(id) {
                placed.push(Err(Refusal::Duplicate));
                continue;
            }
            let idx = self.leaves.len() as u64;
            tx.execute(
                "INSERT INTO sparks (epoch, idx, id, proposal_id, miner, nonce, weight)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    self.epoch as i64,
                    idx as i64,
                    id.to_vec(),
                    s.proposal_id.to_vec(),
                    s.miner.to_vec(),
                    s.nonce as i64,
                    (*weight).min(i64::MAX as u64) as i64
                ],
            )
            .map_err(err)?;
            self.leaves.push(leaf_hash(&s.leaf(self.epoch)));
            placed.push(Ok((*s, idx)));
        }
        tx.commit().map_err(err)?;
        let sth = self.sign_head(false)?;
        Ok(placed
            .into_iter()
            .map(|p| {
                p.map(|(spark, idx)| Receipt {
                    spark,
                    leaf_index: idx,
                    sth,
                    path: inclusion_proof(&self.leaves[..sth.tree_size as usize], idx as usize),
                })
            })
            .collect())
    }

    /// The latest tree head of an epoch's log (the final one once the window closed).
    pub fn head(&self, epoch: u64) -> Result<Option<Sth>, String> {
        self.conn
            .query_row(
                "SELECT tree_size, root, at, signature FROM sths WHERE epoch = ?1
                 ORDER BY final DESC, tree_size DESC, at DESC LIMIT 1",
                [epoch as i64],
                |r| {
                    Ok(Sth {
                        epoch,
                        tree_size: r.get::<_, i64>(0)? as u64,
                        root: blob32(r.get(1)?),
                        timestamp_ms: r.get::<_, i64>(2)? as u64,
                        signature: r.get::<_, Vec<u8>>(3)?.try_into().unwrap_or([0; 64]),
                    })
                },
            )
            .optional()
            .map_err(err)
    }

    fn epoch_leaves(&self, epoch: u64) -> Result<Vec<Hash>, String> {
        if epoch == self.epoch {
            return Ok(self.leaves.clone());
        }
        let mut stmt = self
            .conn
            .prepare("SELECT proposal_id, miner, nonce FROM sparks WHERE epoch = ?1 ORDER BY idx")
            .map_err(err)?;
        let rows = stmt
            .query_map([epoch as i64], |r| {
                Ok(Spark {
                    proposal_id: blob32(r.get(0)?),
                    miner: blob32(r.get(1)?),
                    nonce: r.get::<_, i64>(2)? as u64,
                })
            })
            .map_err(err)?;
        rows.map(|r| r.map(|s| leaf_hash(&s.leaf(epoch))).map_err(err))
            .collect()
    }

    /// An inclusion proof for leaf `index` in the epoch's tree of `size` leaves.
    pub fn inclusion(
        &self,
        epoch: u64,
        index: u64,
        size: u64,
    ) -> Result<Option<Vec<Hash>>, String> {
        let leaves = self.epoch_leaves(epoch)?;
        if size == 0 || size as usize > leaves.len() || index >= size {
            return Ok(None);
        }
        Ok(Some(inclusion_proof(
            &leaves[..size as usize],
            index as usize,
        )))
    }

    /// A consistency proof between the epoch's trees of `first` and `second` leaves.
    pub fn consistency(
        &self,
        epoch: u64,
        first: u64,
        second: u64,
    ) -> Result<Option<Vec<Hash>>, String> {
        let leaves = self.epoch_leaves(epoch)?;
        if first == 0 || first > second || second as usize > leaves.len() {
            return Ok(None);
        }
        Ok(Some(consistency_proof(
            &leaves[..second as usize],
            first as usize,
        )))
    }

    pub fn wishes(&self, status: Option<&str>, limit: u32) -> Result<Vec<Value>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, bytes, author, action, created_epoch, expires_epoch, status, work, reason, executed_epoch FROM wishes
                 WHERE (?1 IS NULL OR status = ?1) ORDER BY at DESC LIMIT ?2",
            )
            .map_err(err)?;
        let rows = stmt
            .query_map(params![status, limit], |r| {
                Ok(json!({
                    "id": hex(&r.get::<_, Vec<u8>>(0)?),
                    "bytes": hex(&r.get::<_, Vec<u8>>(1)?),
                    "author": hex(&r.get::<_, Vec<u8>>(2)?),
                    "action": action_name(r.get::<_, i64>(3)?),
                    "created_epoch": r.get::<_, i64>(4)?,
                    "expires_epoch": r.get::<_, i64>(5)?,
                    "status": r.get::<_, String>(6)?,
                    "work": r.get::<_, String>(7)?,
                    "reason": r.get::<_, Option<String>>(8)?,
                    "executed_epoch": r.get::<_, Option<i64>>(9)?,
                    "price_mult": PRICE_MULT.get(r.get::<_, i64>(3)? as usize).copied().unwrap_or(100) as u64,
                }))
            })
            .map_err(err)?;
        rows.map(|r| r.map_err(err)).collect()
    }
}

fn action_name(code: i64) -> &'static str {
    match code {
        0 => "weather",
        1 => "migrate",
        2 => "revive",
        _ => "?",
    }
}

fn blob32(v: Vec<u8>) -> [u8; 32] {
    v.try_into().unwrap_or([0; 32])
}

pub fn sth_json(s: &Sth) -> Value {
    json!({
        "epoch": s.epoch,
        "tree_size": s.tree_size,
        "root": hex(&s.root),
        "timestamp_ms": s.timestamp_ms,
        "signature": hex(&s.signature),
    })
}

pub fn receipt_json(r: &Receipt) -> Value {
    json!({
        "spark_id": hex(&r.spark.id(r.sth.epoch)),
        "leaf_index": r.leaf_index,
        "sth": sth_json(&r.sth),
        "path": r.path.iter().map(|h| hex(h)).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use protogaea_pow::{Hasher, SPARK};
    use protogaea_protocol::wish::{Action, Weather};

    fn find_spark(
        h: &mut Hasher,
        t: &Ticket,
        proposal_id: Hash,
        miner: [u8; 32],
        from: u64,
    ) -> (Spark, u64) {
        let mut s = Spark {
            proposal_id,
            miner,
            nonce: from,
        };
        loop {
            if let Some(w) = s.check(h, &t.world_id, t.epoch, &t.challenge, t.target) {
                return (s, w);
            }
            s.nonce += 1;
        }
    }

    /// A wish with its sparks through a window: receipts that verify, duplicates and a closed
    /// window refused, work added at the close, and the log's proofs.
    #[test]
    fn a_window_from_open_to_close() {
        let dir = std::env::temp_dir().join(format!("protogaea-intake-{}", now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut intake = Intake::open(&dir, [1; 16], [2; 32], 1_000_000).unwrap();
        // An easy target, so the test finds sparks quickly.
        intake.conn.execute("INSERT INTO windows (epoch, challenge, target, accepted) VALUES (9, x'00', ?1, 20000)", [(u64::MAX / 4) as i64]).unwrap();
        intake.open_window(10, &[3; 32]).unwrap();
        assert!(
            intake.target > u64::MAX / 8,
            "the target follows the last window"
        );

        let author_secret = [5u8; 32];
        let w = Wish {
            world_id: [1; 16],
            ruleset_id: [2; 32],
            action: Action::Weather {
                x: 1,
                y: 2,
                kind: Weather::Rain,
            },
            author: wish::public_key(&author_secret),
            created_epoch: 10,
            expires_epoch: 11,
            hypothesis: None,
            name: None,
        };
        let bytes = w.to_bytes();
        let sig = wish::sign(&author_secret, &w.id());
        let bad_sig = wish::sign(&[6; 32], &w.id());
        assert_eq!(
            intake.precheck_wish(&bytes, &bad_sig).err(),
            Some(Refusal::Signature)
        );
        let checked = intake.precheck_wish(&bytes, &sig).unwrap();
        intake.add_wish(&checked, &bytes, &sig).unwrap();
        assert_eq!(
            intake.precheck_wish(&bytes, &sig).err(),
            Some(Refusal::Duplicate)
        );

        let mut h = Hasher::new(SPARK);
        let t = intake.ticket().unwrap();
        let a = find_spark(&mut h, &t, w.id(), [7; 32], 0);
        let b = find_spark(&mut h, &t, w.id(), [7; 32], a.0.nonce + 1);
        let receipts = intake.append(&t, &[a, b]).unwrap();
        for r in &receipts {
            assert!(r.as_ref().unwrap().verify(&intake.operator));
        }
        assert_eq!(intake.precheck(&a.0), Err(Refusal::Duplicate));
        let stranger = Spark {
            proposal_id: [9; 32],
            ..a.0
        };
        assert_eq!(intake.precheck(&stranger), Err(Refusal::UnknownProposal));

        // Consistency between the head after one spark and the head after two.
        let first_root = root(&intake.leaves[..1]);
        let proof = intake.consistency(10, 1, 2).unwrap().unwrap();
        let head = intake.head(10).unwrap().unwrap();
        assert!(protogaea_protocol::log::verify_consistency(
            1,
            2,
            &first_root,
            &head.root,
            &proof
        ));

        let selected = intake.close_window(&[0; 32]).unwrap();
        assert!(selected.is_empty(), "far below the price");
        let fin = intake.head(10).unwrap().unwrap();
        assert!(fin.verify(&intake.operator));
        assert_eq!(fin.tree_size, 2);
        assert_eq!(intake.ticket().err(), Some(Refusal::WindowClosed));
        assert_eq!(
            intake.append(&t, &[b]).unwrap()[0].as_ref().err(),
            Some(&Refusal::WindowClosed)
        );
        let wishes = intake.wishes(None, 10).unwrap();
        assert_eq!(wishes[0]["work"], (a.1 + b.1).to_string());
        assert_eq!(wishes[0]["status"], "open");

        // The next window: a new challenge, and at its close the wish expires.
        intake.open_window(11, &[4; 32]).unwrap();
        assert_ne!(intake.challenge, t.challenge);
        intake.close_window(&[0; 32]).unwrap();
        assert_eq!(intake.wishes(Some("expired"), 10).unwrap().len(), 1);
        drop(intake);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
