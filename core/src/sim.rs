//! One epoch of the world (spec §13): the epoch boundary, then twelve ticks.
//!
//! Stage A1 has no epoch-boundary steps yet: rifts, natural events, miracles and natural
//! revival arrive in stage A2, so an epoch is simply twelve ticks.

use crate::genome::{mutate, FERTILITY, HUNTING, PLANT};
use crate::rng::{derive, Purpose, Rng};
use crate::ruleset::Ruleset;
use crate::state::{Biome, Clade, Organism, World};

/// Why an organism died.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathCause {
    Starvation,
    OldAge,
    Predation,
}

/// What happened during one epoch. Not part of consensus: it is derived from the run and
/// used for statistics and, later, for the event log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EpochReport {
    pub births: u32,
    pub deaths_starvation: u32,
    pub deaths_old_age: u32,
    pub deaths_predation: u32,
    pub attacks: u32,
    pub births_blocked_space: u32,
    pub births_blocked_cap: u32,
    /// Ticks in which at least one birth was blocked by the global limit.
    pub ticks_at_cap: u32,
    /// `(parent_id, child_id)`, in the order the births happened.
    pub births_list: Vec<(u64, u64)>,
    pub clades_founded: Vec<u32>,
    /// Clades whose last member died, as they were at that moment.
    pub clades_extinct: Vec<Clade>,
}

/// `BLAKE3("PROTOGAEA/EPOCH_SEED/V0" ‖ world_id ‖ E ‖ beacon_E ‖ header_hash_{E−1})` (spec §14).
pub fn epoch_seed(
    world_id: &[u8; 16],
    epoch: u64,
    beacon: &[u8; 32],
    prev_header_hash: &[u8; 32],
) -> [u8; 32] {
    derive(
        b"PROTOGAEA/EPOCH_SEED/V0",
        &[world_id, &epoch.to_le_bytes(), beacon, prev_header_hash],
    )
}

/// Runs one epoch.
pub fn step_epoch(world: &mut World, rules: &Ruleset, epoch_seed: &[u8; 32]) -> EpochReport {
    let rng = Rng::new(epoch_seed);
    let mut report = EpochReport::default();
    let mut scratch = Scratch::default();
    for tick in 0..rules.ticks_per_epoch {
        run_tick(world, rules, &rng, tick, &mut report, &mut scratch);
    }
    world.epoch = world.epoch.checked_add(1).expect("epoch counter overflow");
    report
}

const EMPTY: u32 = u32::MAX;

/// Neighbor order for placing newborns. Fixed, because it is part of consensus.
const NEIGHBORS: [(i32, i32); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// Working memory of one tick. Not part of the state.
#[derive(Default)]
struct Scratch {
    /// Organisms per cell.
    occ: Vec<u8>,
    /// Indexes of the organisms in each cell, in arrival order.
    members: Vec<[u32; 4]>,
    /// The strongest hunter in each cell at the start of the tick: (attack, clade).
    hunter: Vec<(i32, u32)>,
    /// The weakest organism in each cell at the start of the tick: (defense, clade).
    prey: Vec<(i32, u32)>,
    /// (queue key, organism id, organism index).
    queue: Vec<(u64, u64, u32)>,
    alive: Vec<bool>,
    /// Energy spent on movement and attacks during the tick.
    spent: Vec<i32>,
    alive_count: usize,
}

impl Scratch {
    fn reset(&mut self, cells: usize, organisms: usize) {
        self.occ.clear();
        self.occ.resize(cells, 0);
        self.members.clear();
        self.members.resize(cells, [EMPTY; 4]);
        self.hunter.clear();
        self.hunter.resize(cells, (i32::MIN, 0));
        self.prey.clear();
        self.prey.resize(cells, (i32::MAX, 0));
        self.queue.clear();
        self.alive.clear();
        self.alive.resize(organisms, true);
        self.spent.clear();
        self.spent.resize(organisms, 0);
        self.alive_count = organisms;
    }

    fn add(&mut self, cell: usize, index: u32) {
        let n = usize::from(self.occ[cell]);
        assert!(n < 4, "more than four organisms in a cell");
        self.members[cell][n] = index;
        self.occ[cell] += 1;
    }

    fn remove(&mut self, cell: usize, index: u32) {
        let n = usize::from(self.occ[cell]);
        let slots = &mut self.members[cell];
        let pos = slots[..n]
            .iter()
            .position(|&m| m == index)
            .expect("an organism is listed in its cell");
        slots.copy_within(pos + 1..n, pos);
        slots[n - 1] = EMPTY;
        self.occ[cell] -= 1;
    }
}

fn run_tick(
    world: &mut World,
    rules: &Ruleset,
    rng: &Rng,
    tick: u32,
    report: &mut EpochReport,
    s: &mut Scratch,
) {
    // 1. Environment: decomposition and food growth.
    for cell in &mut world.cells {
        let p = rules.biomes[cell.biome as usize];
        if !p.passable {
            continue;
        }
        let decomposed =
            (u64::from(cell.detritus) * u64::from(rules.decomposition_pct) / 100) as u32;
        cell.detritus -= decomposed;
        cell.food = cell
            .food
            .saturating_add(decomposed)
            .saturating_add(p.base_regen)
            .min(p.food_max);
    }

    // Index organisms by cell, and note what each cell looks like to its neighbors.
    s.reset(world.cells.len(), world.organisms.len());
    for (i, o) in world.organisms.iter().enumerate() {
        let c = usize::from(o.cell);
        s.add(c, i as u32);
        if o.genome.traits[HUNTING] > 0 {
            let power = o.genome.attack(rules);
            if power > s.hunter[c].0 {
                s.hunter[c] = (power, o.clade_id);
            }
        }
        let defense = o.genome.defense(rules) + cover(rules, world.cells[c].biome);
        if defense < s.prey[c].0 {
            s.prey[c] = (defense, o.clade_id);
        }
    }

    // 2. Queue: a stable permutation by H(epoch_seed, tick, organism_id).
    for (i, o) in world.organisms.iter().enumerate() {
        s.queue
            .push((rng.raw(tick, Purpose::Queue, o.id, 0), o.id, i as u32));
    }
    s.queue.sort_unstable();

    // 3. Actions, in queue order. An organism that died before its turn does not act.
    for q in 0..s.queue.len() {
        let i = s.queue[q].2 as usize;
        if s.alive[i] {
            act(world, rules, rng, tick, i, report, s);
        }
    }

    // 4. Energy costs; deaths from starvation and old age.
    for i in 0..world.organisms.len() {
        if !s.alive[i] {
            continue;
        }
        let o = world.organisms[i];
        let biome = world.cells[usize::from(o.cell)].biome;
        let mut cost = o.genome.upkeep(rules);
        if let Some(preferred) = Biome::from_habitat(o.genome.habitat) {
            let delta = cost * rules.habitat_modifier_pct / 100;
            cost = if biome == preferred {
                cost - delta
            } else {
                cost + delta
            };
        }
        if biome == Biome::Shallows {
            cost += rules.shallow_drain;
        }
        let energy = o.energy - cost - s.spent[i];
        world.organisms[i].energy = energy;
        if energy <= 0 {
            kill(world, rules, s, report, i, DeathCause::Starvation);
            continue;
        }
        let age = o.age + 1;
        world.organisms[i].age = age;
        if age >= rules.max_age {
            kill(world, rules, s, report, i, DeathCause::OldAge);
        } else if age > rules.senescence_start {
            let span = u64::from(rules.max_age - rules.senescence_start);
            let ppm = (u64::from(age - rules.senescence_start) * 1_000_000 / span) as u32;
            if rng.chance_ppm(tick, Purpose::Senescence, o.id, ppm) {
                kill(world, rules, s, report, i, DeathCause::OldAge);
            }
        }
    }

    // 5. Births, mutations, clades.
    births(world, rules, rng, tick, report, s);

    // 6. Drop the dead (ids stay in order) and close extinct clades. Statistics are derived
    // outside consensus.
    let mut k = 0;
    world.organisms.retain(|_| {
        let keep = s.alive[k];
        k += 1;
        keep
    });
    let extinct: Vec<u32> = world
        .clades
        .values()
        .filter(|c| c.living == 0)
        .map(|c| c.id)
        .collect();
    for id in extinct {
        if let Some(clade) = world.clades.remove(&id) {
            report.clades_extinct.push(clade);
        }
    }
}

/// Movement, then an attack or a meal (spec §11.2–11.4).
fn act(
    world: &mut World,
    rules: &Ruleset,
    rng: &Rng,
    tick: u32,
    i: usize,
    report: &mut EpochReport,
    s: &mut Scratch,
) {
    let me = world.organisms[i];
    let mut pos = usize::from(me.cell);
    let steps = me.genome.steps();
    if steps > 0 {
        let target = choose_target(world, rules, rng, tick, &me, s);
        if target != pos {
            pos = walk(world, rules, s, i, pos, target, steps);
            world.organisms[i].cell = pos as u16;
        }
    }

    let mut attacked = false;
    if is_hungry_hunter(&me, rules) {
        if let Some(j) = choose_prey(world, rules, s, i, &me, pos) {
            attacked = true;
            report.attacks += 1;
            s.spent[i] += rules.attack_cost;
            let span = u64::from(rules.roll_span) + 1;
            let attack =
                me.genome.attack(rules) + rng.below(tick, Purpose::AttackRoll, me.id, span) as i32;
            let prey = world.organisms[j];
            let defense = prey.genome.defense(rules)
                + cover(rules, world.cells[usize::from(prey.cell)].biome)
                + rng.below(tick, Purpose::DefenseRoll, me.id, span) as i32;
            if attack > defense {
                kill(world, rules, s, report, j, DeathCause::Predation);
                let gain =
                    prey.energy.max(0) * rules.predation_efficiency_pct / 100 + rules.body_value;
                let energy = &mut world.organisms[i].energy;
                *energy = (*energy + gain).min(rules.energy_max);
            }
        }
    }

    if !attacked && me.genome.traits[PLANT] > 0 {
        let bite = u32::from(me.genome.traits[PLANT]) * rules.bite_per_point;
        let cell = &mut world.cells[pos];
        let eaten = cell.food.min(bite);
        cell.food -= eaten;
        let energy = &mut world.organisms[i].energy;
        *energy = (*energy + eaten as i32 * rules.plant_efficiency).min(rules.energy_max);
    }
}

/// The best cell within sight (spec §11.4). Ties are broken by counter-based randomness.
fn choose_target(
    world: &World,
    rules: &Ruleset,
    rng: &Rng,
    tick: u32,
    me: &Organism,
    s: &Scratch,
) -> usize {
    let g = &me.genome;
    let here = usize::from(me.cell);
    let (x0, y0) = world.coords(here);
    let radius = g.sight();
    let w = &rules.weights;
    let bite = u32::from(g.traits[PLANT]) * rules.bite_per_point;
    let hunter = is_hungry_hunter(me, rules);
    let my_attack = g.attack(rules);
    let my_defense = g.defense(rules);
    let preferred = Biome::from_habitat(g.habitat);
    let caution = 4 - i64::from(g.boldness);

    let mut best = here;
    let mut best_score = i64::MIN;
    let mut best_tie: Option<u64> = None;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let Some(c) = world.index(x0 + dx, y0 + dy) else {
                continue;
            };
            let cell = world.cells[c];
            let p = rules.biomes[cell.biome as usize];
            if !p.passable || (c != here && s.occ[c] >= rules.max_per_cell) {
                continue;
            }
            let others = i64::from(s.occ[c]) - i64::from(c == here);
            let distance = i64::from(dx.abs().max(dy.abs()));

            let mut score = 0i64;
            if bite > 0 {
                score += i64::from(cell.food.min(bite))
                    * i64::from(rules.plant_efficiency)
                    * i64::from(w.food_pct)
                    / 100;
            }
            if hunter {
                let (defense, clade) = s.prey[c];
                if defense != i32::MAX && clade != me.clade_id && my_attack > defense {
                    score += i64::from(my_attack - defense) * i64::from(w.hunt_per_point);
                }
            }
            let threat = strongest_threat(world, s, c, me.clade_id, my_defense + p.cover);
            score -= threat * i64::from(w.danger_per_point) * caution / 4;
            if preferred == Some(cell.biome) {
                score += i64::from(w.habitat_bonus);
            }
            score -= others * i64::from(w.crowd_per_neighbor);
            score -= distance * i64::from(p.move_cost);
            score += distance * i64::from(g.dispersal) * i64::from(w.dispersal_per_step);

            if score > best_score {
                best = c;
                best_score = score;
                best_tie = None;
            } else if score == best_score {
                let current = *best_tie
                    .get_or_insert_with(|| rng.raw(tick, Purpose::MoveTie, me.id, best as u32));
                let candidate = rng.raw(tick, Purpose::MoveTie, me.id, c as u32);
                if candidate < current {
                    best = c;
                    best_tie = Some(candidate);
                }
            }
        }
    }
    best
}

/// The defense bonus of standing in a biome (spec §11.3).
fn cover(rules: &Ruleset, biome: Biome) -> i32 {
    rules.biomes[biome as usize].cover
}

/// Hunters hunt only while hungry (satiation), which keeps them from wiping out their prey.
fn is_hungry_hunter(me: &Organism, rules: &Ruleset) -> bool {
    me.genome.traits[HUNTING] > 0 && me.energy < rules.energy_max * rules.hunt_hunger_pct / 100
}

/// The largest attack margin of a non-kin hunter within one cell of `c`, or 0.
fn strongest_threat(world: &World, s: &Scratch, c: usize, my_clade: u32, my_defense: i32) -> i64 {
    let (x, y) = world.coords(c);
    let mut threat = 0i64;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let Some(n) = world.index(x + dx, y + dy) else {
                continue;
            };
            let (power, clade) = s.hunter[n];
            if power != i32::MIN && clade != my_clade {
                threat = threat.max(i64::from(power - my_defense));
            }
        }
    }
    threat
}

/// Moves towards `target` for at most `steps` cells. Each step shortens the Chebyshev
/// distance; directions are tried in a fixed order.
fn walk(
    world: &World,
    rules: &Ruleset,
    s: &mut Scratch,
    i: usize,
    from: usize,
    target: usize,
    steps: u32,
) -> usize {
    let (tx, ty) = world.coords(target);
    let mut pos = from;
    for _ in 0..steps {
        if pos == target {
            break;
        }
        let (x, y) = world.coords(pos);
        let (sx, sy) = ((tx - x).signum(), (ty - y).signum());
        let mut next = None;
        for (nx, ny) in [(x + sx, y + sy), (x + sx, y), (x, y + sy)] {
            if (nx, ny) == (x, y) {
                continue;
            }
            let Some(n) = world.index(nx, ny) else {
                continue;
            };
            if rules.biomes[world.cells[n].biome as usize].passable && s.occ[n] < rules.max_per_cell
            {
                next = Some(n);
                break;
            }
        }
        let Some(n) = next else {
            break;
        };
        s.remove(pos, i as u32);
        s.add(n, i as u32);
        s.spent[i] += rules.biomes[world.cells[n].biome as usize].move_cost;
        pos = n;
    }
    pos
}

/// The non-kin organism nearby with the best expected margin, if any is worth attacking.
fn choose_prey(
    world: &World,
    rules: &Ruleset,
    s: &Scratch,
    i: usize,
    me: &Organism,
    pos: usize,
) -> Option<usize> {
    let my_attack = me.genome.attack(rules);
    let (x, y) = world.coords(pos);
    let mut best: Option<(i32, u64, usize)> = None;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let Some(n) = world.index(x + dx, y + dy) else {
                continue;
            };
            for &m in &s.members[n][..usize::from(s.occ[n])] {
                let j = m as usize;
                if j == i || !s.alive[j] {
                    continue;
                }
                let other = &world.organisms[j];
                if other.genome.distance(&me.genome) <= rules.kin_distance {
                    continue;
                }
                let margin =
                    my_attack - (other.genome.defense(rules) + cover(rules, world.cells[n].biome));
                if margin < 0 {
                    continue;
                }
                let better = match best {
                    None => true,
                    Some((best_margin, best_id, _)) => {
                        margin > best_margin || (margin == best_margin && other.id < best_id)
                    }
                };
                if better {
                    best = Some((margin, other.id, j));
                }
            }
        }
    }
    best.map(|(_, _, j)| j)
}

fn kill(
    world: &mut World,
    rules: &Ruleset,
    s: &mut Scratch,
    report: &mut EpochReport,
    i: usize,
    cause: DeathCause,
) {
    debug_assert!(s.alive[i]);
    s.alive[i] = false;
    s.alive_count -= 1;
    let o = world.organisms[i];
    let cell = usize::from(o.cell);
    s.remove(cell, i as u32);
    world
        .clades
        .get_mut(&o.clade_id)
        .expect("every organism has a clade")
        .living -= 1;
    let detritus = match cause {
        DeathCause::Predation => rules.remains_detritus,
        DeathCause::Starvation | DeathCause::OldAge => rules.body_detritus,
    };
    world.cells[cell].detritus = world.cells[cell].detritus.saturating_add(detritus);
    match cause {
        DeathCause::Starvation => report.deaths_starvation += 1,
        DeathCause::OldAge => report.deaths_old_age += 1,
        DeathCause::Predation => report.deaths_predation += 1,
    }
}

/// Each organism with enough energy has one offspring (spec §11.5). Newborns do not act or
/// breed in the tick they are born.
fn births(
    world: &mut World,
    rules: &Ruleset,
    rng: &Rng,
    tick: u32,
    report: &mut EpochReport,
    s: &mut Scratch,
) {
    let parents = world.organisms.len();
    let mut blocked_by_cap = false;
    for i in 0..parents {
        if !s.alive[i] {
            continue;
        }
        let parent = world.organisms[i];
        let fertility = i32::from(parent.genome.traits[FERTILITY]);
        if parent.energy < rules.repro_base - fertility * rules.repro_per_fertility {
            continue;
        }
        if s.alive_count >= rules.max_organisms as usize {
            report.births_blocked_cap += 1;
            blocked_by_cap = true;
            continue;
        }
        let child_id = world.next_organism_id;
        let home = usize::from(parent.cell);
        let site = if s.occ[home] < rules.max_per_cell {
            Some(home)
        } else {
            let (x, y) = world.coords(home);
            let mut options = [0usize; 8];
            let mut n = 0;
            for (dx, dy) in NEIGHBORS {
                if let Some(c) = world.index(x + dx, y + dy) {
                    if rules.biomes[world.cells[c].biome as usize].passable
                        && s.occ[c] < rules.max_per_cell
                    {
                        options[n] = c;
                        n += 1;
                    }
                }
            }
            (n > 0)
                .then(|| options[rng.below(tick, Purpose::Placement, child_id, n as u64) as usize])
        };
        let Some(site) = site else {
            report.births_blocked_space += 1;
            continue;
        };

        let genome = mutate(&parent.genome, rules, rng, tick, child_id);
        let child_energy = rules.child_base - fertility * rules.child_per_fertility;
        world.organisms[i].energy -= child_energy + rules.birth_cost;

        let reference = world
            .clades
            .get(&parent.clade_id)
            .expect("every organism has a clade")
            .reference;
        let clade_id = if genome.distance(&reference) >= rules.clade_split_distance {
            let id = world.next_clade_id;
            world.next_clade_id = id.checked_add(1).expect("clade id overflow");
            world.clades.insert(
                id,
                Clade {
                    id,
                    parent_id: parent.clade_id,
                    reference: genome,
                    founded_epoch: world.epoch,
                    living: 0,
                    peak_living: 0,
                },
            );
            report.clades_founded.push(id);
            id
        } else {
            parent.clade_id
        };
        let clade = world.clades.get_mut(&clade_id).expect("the clade exists");
        clade.living += 1;
        clade.peak_living = clade.peak_living.max(clade.living);

        world.next_organism_id = child_id.checked_add(1).expect("organism id overflow");
        world.organisms.push(Organism {
            id: child_id,
            parent_id: parent.id,
            lineage_id: parent.lineage_id,
            clade_id,
            cell: site as u16,
            age: 0,
            energy: child_energy,
            genome,
        });
        let index = world.organisms.len() - 1;
        s.alive.push(true);
        s.spent.push(0);
        s.add(site, index as u32);
        s.alive_count += 1;
        report.births += 1;
        report.births_list.push((parent.id, child_id));
    }
    if blocked_by_cap {
        report.ticks_at_cap += 1;
    }
}
