//! Patron bots (spec v0.3, draft, §11): simulated players who back clades with sparks, to see
//! whether the world holds when everyone helps the leader, when a whale backs one lineage, when a
//! crowd harasses the smallest clades, or when two camps wage war.
//!
//! The economy is the ledger of spec §19 reduced to what the question needs: each group of bots
//! puts its share of the work of an epoch into its one open wish; a wish is ready when its work
//! covers its price, which is a base price × the action's multiplier × the share multiplier of the
//! clade it names (spec v0.3 §6); at most three ready wishes, the best covered first, go to the
//! world each epoch. A refusal that passes by itself keeps the wish waiting; any other burns it.
//!
//! Helping groups also cross their clade with a compatible neighbour (`hybrid`, spec v0.3 §5); the
//! report follows how many hybrid clades, with their descendants, live a day and three days on.

use std::collections::BTreeMap;

use protogaea_core::genome::GIFTS;
use protogaea_core::miracle::{
    is_harm, is_transient, share_mult, BLIGHT, CURE, EXPOSE, FORAGE, GIFT, SHELTER, SICKNESS,
};
use protogaea_core::{Miracle, Ruleset, World};

/// How the bots choose what to back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    /// No players: the baseline.
    None,
    /// Everyone helps the largest clade.
    Leader,
    /// Everyone helps the smallest clade that can take help.
    Weak,
    /// Random clades, random help.
    Random,
    /// One bot with half of all the work helps one lineage all season; the rest is random.
    Whale,
    /// Everyone harms the smallest clade that can be harmed.
    Harass,
    /// Two camps back two lineages, each helping its own and harming the other's.
    War,
    /// 70% random, 20% for the leader, 10% a whale.
    Mixed,
}

impl Strategy {
    pub const ALL: [Strategy; 8] = [
        Strategy::None,
        Strategy::Leader,
        Strategy::Weak,
        Strategy::Random,
        Strategy::Whale,
        Strategy::Harass,
        Strategy::War,
        Strategy::Mixed,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Strategy::None => "none",
            Strategy::Leader => "leader",
            Strategy::Weak => "weak",
            Strategy::Random => "random",
            Strategy::Whale => "whale",
            Strategy::Harass => "harass",
            Strategy::War => "war",
            Strategy::Mixed => "mixed",
        }
    }

    pub fn parse(s: &str) -> Result<Strategy, String> {
        Strategy::ALL
            .into_iter()
            .find(|k| k.name() == s)
            .ok_or_else(|| format!("unknown strategy `{s}`; one of none, leader, weak, random, whale, harass, war, mixed"))
    }
}

/// What a group of bots does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Leader,
    Weak,
    Random,
    /// Helps the largest clade of its lineage.
    Champion(u32),
    Harass,
    /// Helps its lineage's largest clade and harms the rival lineage's, in turn.
    Camp {
        own: u32,
        rival: u32,
    },
}

struct Group {
    role: Role,
    /// Work units a epoch.
    rate: u64,
    wish: Option<Wish>,
    /// Alternates help and harm for a camp.
    turn: u64,
}

#[derive(Clone, Copy, Debug)]
struct Wish {
    miracle: Plan,
    work: u64,
    waited: u64,
}

/// The bots' code for a hybrid wish (the core's miracle is `Miracle::Hybrid`).
const HYBRID: u8 = 255;

/// A wish's miracle: (action, clade, center, partner clade of a hybrid or 0).
type Plan = (u8, u32, u16, u32);

/// Prices (spec v0.3 §6, candidates), in the bots' work units.
const BASE_PRICE: u64 = 100;
fn action_mult(action: u8) -> u64 {
    if action == HYBRID {
        250
    } else if action >= GIFT {
        150
    } else if is_harm(action) {
        120
    } else {
        100
    }
}

/// A small generator for the bots' choices; outside consensus.
struct Dice(u64);
impl Dice {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

/// Per-clade facts the bots read each epoch.
struct Census {
    living: BTreeMap<u32, u32>,
    lineage: BTreeMap<u32, u32>,
    population: u64,
}

impl Census {
    fn of(world: &World) -> Census {
        let mut living = BTreeMap::new();
        let mut lineage = BTreeMap::new();
        for o in &world.organisms {
            *living.entry(o.clade_id).or_insert(0) += 1;
            lineage.entry(o.clade_id).or_insert(o.lineage_id);
        }
        Census {
            living,
            lineage,
            population: world.organisms.len() as u64,
        }
    }

    fn share(&self, clade: u32) -> u64 {
        u64::from(self.living.get(&clade).copied().unwrap_or(0)) * 1000 / self.population.max(1)
    }

    fn by_size(&self) -> Vec<(u32, u32)> {
        let mut v: Vec<(u32, u32)> = self.living.iter().map(|(&c, &n)| (n, c)).collect();
        v.sort_by(|a, b| b.cmp(a));
        v.into_iter().map(|(n, c)| (c, n)).collect()
    }

    fn largest_of_lineage(&self, lineage: u32) -> Option<u32> {
        self.by_size()
            .into_iter()
            .find(|(c, _)| self.lineage.get(c) == Some(&lineage))
            .map(|(c, _)| c)
    }
}

/// The cell with the most members of a clade within `radius`, and how many.
fn densest(world: &World, clade: u32, radius: i32) -> (u16, u32) {
    let (w, h) = (i32::from(world.width), i32::from(world.height));
    let mut grid = vec![0u32; world.cells.len()];
    for o in world.organisms.iter().filter(|o| o.clade_id == clade) {
        grid[usize::from(o.cell)] += 1;
    }
    // Box sums by prefix sums.
    let idx = |x: i32, y: i32| (y * (w + 1) + x) as usize;
    let mut pre = vec![0u32; ((w + 1) * (h + 1)) as usize];
    for y in 0..h {
        for x in 0..w {
            pre[idx(x + 1, y + 1)] =
                grid[(y * w + x) as usize] + pre[idx(x, y + 1)] + pre[idx(x + 1, y)]
                    - pre[idx(x, y)];
        }
    }
    let mut best = (0u16, 0u32);
    for y in 0..h {
        for x in 0..w {
            if grid[(y * w + x) as usize] == 0 {
                continue;
            }
            let (x0, y0) = ((x - radius).max(0), (y - radius).max(0));
            let (x1, y1) = ((x + radius + 1).min(w), (y + radius + 1).min(h));
            let n = pre[idx(x1, y1)] + pre[idx(x0, y0)] - pre[idx(x0, y1)] - pre[idx(x1, y0)];
            if n > best.1 {
                best = ((y * w + x) as u16, n);
            }
        }
    }
    best
}

/// What happened, for the report.
#[derive(Clone, Debug, Default)]
pub struct Tally {
    pub applied_help: u32,
    pub applied_harm: u32,
    pub applied_gifts: u32,
    pub refused: BTreeMap<&'static str, u32>,
    /// Gifts given, and those still held in the gifted lineage 3 world days later.
    pub gift_events: [u32; 6],
    pub gift_kept: [u32; 6],
    /// Named clades that went extinct.
    pub named_extinct: u32,
    /// The longest time a whale's or a camp's lineage held more than 60% of the living, in days.
    pub backed_over_60_days: f64,
    /// The two lineages that lead after the first world day (a war's camps A and B) and each
    /// one's living at the end; tracked under every strategy, to compare a war with no players.
    pub war: Option<(u32, u32, u32, u32)>,
    /// The backed lineage's share at the end, per mille.
    pub backed_final_permille: u64,
    /// At the end, under v0.3 traits: burrowers, swimmers, scavengers by birth, giants (size 6+),
    /// and the mean size and longevity ×10.
    pub niches: [u32; 4],
    pub size_x10: u32,
    pub longevity_x10: u32,
    /// Hybrid clades founded, and of those checked a day and three days on, how many were
    /// alive then (the clade or a clade descended from it). To compare with, slots 2 and 3 are
    /// the same for a sample of clades founded by mutation (those of every world hour's first
    /// epoch), and slots 4 and 5 for clades founded by mutation within the last hour that
    /// already have as many members as a hybrid clade starts with.
    pub applied_hybrids: u32,
    pub hybrid_checked: [u32; 6],
    pub hybrid_alive: [u32; 6],
}

pub struct Bots {
    groups: Vec<Group>,
    /// Whales and camps choose their lineages after the first world day, when the strong show.
    assigned: bool,
    dice: Dice,
    pending: Vec<(usize, Plan)>,
    /// (epoch due, gift index, lineage) for gifts to check later.
    gift_checks: Vec<(u64, usize, u32)>,
    /// (epoch due, slot of `Tally::hybrid_checked`, clade) to check later.
    hybrid_checks: Vec<(u64, usize, u32)>,
    backed: Option<u32>,
    backed_streak: u64,
    backed_longest: u64,
    pub tally: Tally,
}

impl Bots {
    /// Bots for a strategy, sharing `work` units a epoch, set up on the world as it starts.
    pub fn new(strategy: Strategy, work: u64, seed: u64, world: &World) -> Bots {
        let lineages: Vec<u32> = {
            let mut by: BTreeMap<u32, u32> = BTreeMap::new();
            for o in &world.organisms {
                *by.entry(o.lineage_id).or_insert(0) += 1;
            }
            let mut v: Vec<(u32, u32)> = by.into_iter().map(|(l, n)| (n, l)).collect();
            v.sort_by(|a, b| b.cmp(a));
            v.into_iter().map(|(_, l)| l).collect()
        };
        let top = lineages.first().copied().unwrap_or(0);
        let second = lineages.get(1).copied().unwrap_or(top);
        let group = |role, share: u64| Group {
            role,
            rate: work * share / 100,
            wish: None,
            turn: 0,
        };
        let (groups, backed) = match strategy {
            Strategy::None => (vec![], None),
            Strategy::Leader => (vec![group(Role::Leader, 100)], None),
            Strategy::Weak => (vec![group(Role::Weak, 100)], None),
            Strategy::Random => ((0..4).map(|_| group(Role::Random, 25)).collect(), None),
            Strategy::Whale => (
                vec![
                    group(Role::Champion(top), 50),
                    group(Role::Random, 25),
                    group(Role::Random, 25),
                ],
                Some(top),
            ),
            Strategy::Harass => (vec![group(Role::Harass, 100)], None),
            Strategy::War => (
                vec![
                    group(
                        Role::Camp {
                            own: top,
                            rival: second,
                        },
                        50,
                    ),
                    group(
                        Role::Camp {
                            own: second,
                            rival: top,
                        },
                        50,
                    ),
                ],
                Some(top),
            ),
            Strategy::Mixed => (
                vec![
                    group(Role::Random, 35),
                    group(Role::Random, 35),
                    group(Role::Leader, 20),
                    group(Role::Champion(top), 10),
                ],
                Some(top),
            ),
        };
        let mut tally = Tally::default();
        if strategy == Strategy::War {
            tally.war = Some((top, second, 0, 0));
        }
        Bots {
            groups,
            assigned: false,
            dice: Dice(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1),
            pending: Vec::new(),
            gift_checks: Vec::new(),
            hybrid_checks: Vec::new(),
            backed,
            backed_streak: 0,
            backed_longest: 0,
            tally,
        }
    }

    fn help_action(&mut self) -> u8 {
        match self.dice.below(5) {
            0 => SHELTER,
            1 => FORAGE,
            2 => CURE,
            3 => GIFT + self.dice.below(GIFTS.len() as u64) as u8,
            _ => HYBRID,
        }
    }

    fn harm_action(&mut self) -> u8 {
        [BLIGHT, EXPOSE, SICKNESS][self.dice.below(3) as usize]
    }

    /// A new wish for a group, if it finds a target.
    fn choose(
        &mut self,
        g: usize,
        world: &World,
        rules: &Ruleset,
        census: &Census,
    ) -> Option<Plan> {
        let p = rules.patrons.as_ref()?;
        let r = i32::from(p.target_radius);
        let fits = |clade: u32| {
            let (cell, n) = densest(world, clade, r);
            (n >= p.min_members).then_some(cell)
        };
        // A hybrid's partner: the largest other clade with enough members around the cell and a
        // reference genome within the crossing distances.
        let partner = |clade: u32, cell: u16| -> Option<u32> {
            let reference = world.clades.get(&clade)?.reference;
            let (cx, cy) = world.coords(usize::from(cell));
            let mut near: BTreeMap<u32, u32> = BTreeMap::new();
            for o in &world.organisms {
                let (x, y) = world.coords(usize::from(o.cell));
                if o.clade_id != clade && (x - cx).abs().max((y - cy).abs()) <= r {
                    *near.entry(o.clade_id).or_insert(0) += 1;
                }
            }
            near.into_iter()
                .filter(|&(c, n)| {
                    n >= p.hybrid_min_each
                        && world.clades.get(&c).is_some_and(|k| {
                            let d = k.reference.distance(&reference);
                            (p.hybrid_min_distance..=p.hybrid_max_distance).contains(&d)
                        })
                })
                .max_by_key(|&(c, n)| (n, std::cmp::Reverse(c)))
                .map(|(c, _)| c)
        };
        let order = census.by_size();
        let role = self.groups[g].role;
        if !self.assigned && matches!(role, Role::Champion(_) | Role::Camp { .. }) {
            return None;
        }
        let (action, candidates): (u8, Vec<u32>) = match role {
            Role::Leader => (self.help_action(), order.iter().map(|x| x.0).collect()),
            Role::Weak => (
                self.help_action(),
                order.iter().rev().map(|x| x.0).collect(),
            ),
            Role::Random => {
                let mut c: Vec<u32> = order.iter().map(|x| x.0).collect();
                // A random rotation, then in order: a random clade that can take help.
                let k = self.dice.below(c.len() as u64) as usize;
                c.rotate_left(k);
                (self.help_action(), c)
            }
            Role::Champion(l) => (
                self.help_action(),
                census.largest_of_lineage(l).into_iter().collect(),
            ),
            Role::Harass => {
                let a = self.harm_action();
                (a, order.iter().rev().map(|x| x.0).collect())
            }
            Role::Camp { own, rival } => {
                self.groups[g].turn += 1;
                if self.groups[g].turn.is_multiple_of(2) {
                    (
                        self.help_action(),
                        census.largest_of_lineage(own).into_iter().collect(),
                    )
                } else {
                    let a = self.harm_action();
                    (a, census.largest_of_lineage(rival).into_iter().collect())
                }
            }
        };
        // Only clades the action can reach: large enough to find in an area, and allowed by
        // the share rule; the first 40 of those in the role's order.
        let reachable = candidates.into_iter().filter(|&clade| {
            let living = census.living[&clade];
            living >= p.min_members
                && share_mult(action, census.share(clade), living, rules).is_some()
        });
        for clade in reachable.take(40) {
            if let Some(cell) = fits(clade) {
                if action != HYBRID {
                    return Some((action, clade, cell, 0));
                }
                if let Some(other) = partner(clade, cell) {
                    return Some((action, clade, cell, other));
                }
            }
        }
        None
    }

    /// Whales and camps take the lineages that lead after the first world day.
    fn assign(&mut self, world: &World) {
        let mut by: BTreeMap<u32, u32> = BTreeMap::new();
        for o in &world.organisms {
            *by.entry(o.lineage_id).or_insert(0) += 1;
        }
        let mut v: Vec<(u32, u32)> = by.into_iter().map(|(l, n)| (n, l)).collect();
        v.sort_by(|a, b| b.cmp(a));
        let top = v.first().map_or(0, |x| x.1);
        let second = v.get(1).map_or(top, |x| x.1);
        let mut camps = 0;
        for g in &mut self.groups {
            g.role = match g.role {
                Role::Champion(_) => Role::Champion(top),
                Role::Camp { .. } => {
                    camps += 1;
                    if camps == 1 {
                        Role::Camp {
                            own: top,
                            rival: second,
                        }
                    } else {
                        Role::Camp {
                            own: second,
                            rival: top,
                        }
                    }
                }
                r => r,
            };
        }
        // Every strategy follows the lineage that leads after day one, so that a whale's or a
        // camp's hold on the world compares with how long the leader holds it by itself.
        self.backed = Some(top);
        self.tally.war = Some((top, second, 0, 0));
        self.assigned = true;
    }

    /// The miracles the bots give for the coming epoch.
    pub fn plan(&mut self, world: &World, rules: &Ruleset) -> Vec<Miracle> {
        if !self.assigned && world.epoch >= u64::from(rules.epochs_per_day) {
            self.assign(world);
        }
        let census = Census::of(world);
        let mut ready: Vec<(u64, usize, Plan)> = Vec::new();
        for g in 0..self.groups.len() {
            if self.groups[g].wish.is_none() {
                self.groups[g].wish = self.choose(g, world, rules, &census).map(|m| Wish {
                    miracle: m,
                    work: 0,
                    waited: 0,
                });
            }
            let rate = self.groups[g].rate;
            let Some(w) = self.groups[g].wish.as_mut() else {
                continue;
            };
            w.work += rate;
            w.waited += 1;
            let (action, clade, _, _) = w.miracle;
            let living = census.living.get(&clade).copied().unwrap_or(0);
            match share_mult(action, census.share(clade), living, rules) {
                None => {
                    // The clade grew too large to help, shrank below protection, or died:
                    // the wish can no longer be met; its work burns.
                    self.groups[g].wish = None;
                    *self
                        .tally
                        .refused
                        .entry("the share rule closes the wish")
                        .or_default() += 1;
                }
                Some(mult) => {
                    let price = BASE_PRICE * action_mult(action) * mult / 10_000;
                    if w.work >= price {
                        ready.push((w.work * 10_000 / price.max(1), g, w.miracle));
                    }
                }
            }
        }
        ready.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        ready.truncate(3);
        self.pending = ready.iter().map(|&(_, g, m)| (g, m)).collect();
        self.pending
            .iter()
            .map(|&(_, (action, clade_id, center, other))| {
                if action == HYBRID {
                    Miracle::Hybrid {
                        clade_a: clade_id,
                        clade_b: other,
                        center,
                    }
                } else {
                    Miracle::Clade {
                        action,
                        clade_id,
                        center,
                    }
                }
            })
            .collect()
    }

    /// Reads what became of the miracles and follows up the season's measures.
    pub fn after(&mut self, world: &World, report: &protogaea_core::EpochReport, rules: &Ruleset) {
        let day = u64::from(rules.epochs_per_day);
        for (k, &(g, (action, clade, _, other))) in self.pending.iter().enumerate() {
            if report.miracles.applied.contains(&k) {
                self.groups[g].wish = None;
                if action == HYBRID {
                    self.tally.applied_hybrids += 1;
                    // The new clade, if it lived through its first epoch; otherwise none.
                    let hybrid = report
                        .clades_founded
                        .iter()
                        .copied()
                        .find(|id| {
                            world.clades.get(id).is_some_and(|c| {
                                c.parent_id == clade && c.second_parent_id == other
                            })
                        })
                        .unwrap_or(u32::MAX);
                    self.hybrid_checks.push((world.epoch + day, 0, hybrid));
                    self.hybrid_checks.push((world.epoch + 3 * day, 1, hybrid));
                } else if action >= GIFT {
                    self.tally.applied_gifts += 1;
                    let gi = usize::from(action - GIFT);
                    let lineage = world
                        .organisms
                        .iter()
                        .find(|o| o.clade_id == clade)
                        .map_or(u32::MAX, |o| o.lineage_id);
                    self.gift_checks.push((
                        world.epoch + 3 * u64::from(rules.epochs_per_day),
                        gi,
                        lineage,
                    ));
                } else if is_harm(action) {
                    self.tally.applied_harm += 1;
                } else {
                    self.tally.applied_help += 1;
                }
            } else if let Some((_, why)) = report.miracles.refused.iter().find(|(i, _)| *i == k) {
                *self.tally.refused.entry(why).or_default() += 1;
                if !is_transient(why) {
                    self.groups[g].wish = None;
                }
            }
        }
        // A wish that waits a world day is dropped; its work burns (the wish expires).
        for g in &mut self.groups {
            if g.wish
                .is_some_and(|w| w.waited > u64::from(rules.epochs_per_day))
            {
                g.wish = None;
            }
        }
        self.pending.clear();
        if world.epoch.is_multiple_of(12) {
            for &id in &report.clades_founded {
                let natural = world
                    .clades
                    .get(&id)
                    .or_else(|| report.clades_extinct.iter().find(|c| c.id == id))
                    .is_some_and(|c| c.second_parent_id == 0 && c.parent_id != 0);
                if natural {
                    self.hybrid_checks.push((world.epoch + day, 2, id));
                    self.hybrid_checks.push((world.epoch + 3 * day, 3, id));
                }
            }
            let start = rules.patrons.as_ref().map_or(4, |p| p.hybrid_count);
            for c in world.clades.values() {
                if c.second_parent_id == 0
                    && c.parent_id != 0
                    && c.founded_epoch + 12 > world.epoch
                    && c.living >= start
                {
                    self.hybrid_checks.push((world.epoch + day, 4, c.id));
                    self.hybrid_checks.push((world.epoch + 3 * day, 5, c.id));
                }
            }
        }
        for c in &report.clades_extinct {
            if c.peak_living >= rules.clade_name_threshold {
                self.tally.named_extinct += 1;
            }
        }
        let due: Vec<(u64, usize, u32)> = self
            .gift_checks
            .iter()
            .copied()
            .filter(|&(at, _, _)| at <= world.epoch)
            .collect();
        self.gift_checks.retain(|&(at, _, _)| at > world.epoch);
        for (_, gi, lineage) in due {
            self.tally.gift_events[gi] += 1;
            let bit = GIFTS[gi];
            if world
                .organisms
                .iter()
                .any(|o| o.lineage_id == lineage && o.genome.has(bit))
            {
                self.tally.gift_kept[gi] += 1;
            }
        }
        let due: Vec<(u64, usize, u32)> = self
            .hybrid_checks
            .iter()
            .copied()
            .filter(|&(at, _, _)| at <= world.epoch)
            .collect();
        self.hybrid_checks.retain(|&(at, _, _)| at > world.epoch);
        for (_, slot, hybrid) in due {
            self.tally.hybrid_checked[slot] += 1;
            // Descendants: clades whose parent is the hybrid or one of its descendants (ids grow,
            // so one pass in id order finds them).
            let mut family = vec![hybrid];
            let mut alive = false;
            for c in world.clades.values() {
                if c.id == hybrid || family.contains(&c.parent_id) {
                    family.push(c.id);
                    alive |= c.living > 0;
                }
            }
            self.tally.hybrid_alive[slot] += u32::from(alive);
        }
        if let Some(l) = self.backed {
            let n = world.organisms.iter().filter(|o| o.lineage_id == l).count() as u64;
            let share = n * 1000 / (world.organisms.len() as u64).max(1);
            self.tally.backed_final_permille = share;
            if share > 600 {
                self.backed_streak += 1;
                self.backed_longest = self.backed_longest.max(self.backed_streak);
            } else {
                self.backed_streak = 0;
            }
            self.tally.backed_over_60_days =
                self.backed_longest as f64 / f64::from(rules.epochs_per_day);
        }
        if let Some(t) = &rules.traits8 {
            let mut n = [0u32; 4];
            let (mut size, mut long) = (0u64, 0u64);
            for o in &world.organisms {
                let g = &o.genome;
                n[0] += u32::from(
                    g.traits[0] <= t.burrow_max_movement && g.traits[4] >= t.burrow_min_defense,
                );
                n[1] += u32::from(g.has(protogaea_core::genome::SWIM));
                n[2] += u32::from(
                    g.traits[2] >= t.scavenger_min_plants && g.traits[3] >= t.scavenger_min_hunting,
                );
                n[3] += u32::from(g.extra[0] >= 6);
                size += u64::from(g.extra[0]);
                long += u64::from(g.extra[1]);
            }
            let pop = (world.organisms.len() as u64).max(1);
            self.tally.niches = n;
            self.tally.size_x10 = (size * 10 / pop) as u32;
            self.tally.longevity_x10 = (long * 10 / pop) as u32;
        }
        if let Some((a, b, _, _)) = self.tally.war {
            let count = |l| world.organisms.iter().filter(|o| o.lineage_id == l).count() as u32;
            self.tally.war = Some((a, b, count(a), count(b)));
        }
    }
}
