//! One epoch of the world (spec §13): the epoch boundary, then twelve ticks.
//!
//! The epoch boundary moves the rifts along their schedule, draws natural events (floods,
//! wildfires, great droughts, plague) and lets the spore bank revive a dying world. Miracles
//! arrive in stage B′.

use std::cmp::Reverse;

use crate::climate::{self, SPRING, SUMMER};
use crate::genome::{mutate, FERTILITY, HUNTING, PLANT};
use crate::rng::{derive, Purpose, Rng};
use crate::ruleset::Ruleset;
use crate::state::{Biome, Clade, Effect, EffectKind, MuseumEntry, Organism, RiftPhase, World};

/// Why an organism died.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathCause {
    Starvation,
    OldAge,
    Predation,
    Plague,
    /// Its cell sank and there was no free land within reach.
    Drowned,
}

/// What happened during one epoch. Not part of consensus: it is derived from the run and
/// used for statistics and, later, for the event log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EpochReport {
    pub births: u32,
    pub deaths_starvation: u32,
    pub deaths_old_age: u32,
    pub deaths_predation: u32,
    pub deaths_plague: u32,
    pub deaths_drowned: u32,
    /// Organisms carried off cells that sank.
    pub rescued: u32,
    pub attacks: u32,
    pub births_blocked_space: u32,
    pub births_blocked_cap: u32,
    /// Ticks in which at least one birth was blocked by the global limit.
    pub ticks_at_cap: u32,
    pub floods: u32,
    pub wildfires: u32,
    pub droughts: u32,
    pub plagues: u32,
    /// Rift cells that moved to their next phase.
    pub rift_changes: u32,
    /// Land bridges that closed.
    pub bridges_closed: Vec<u8>,
    /// Whether the spore bank revived the world, and how many organisms it placed.
    pub revival: bool,
    pub revived: u32,
    /// `(parent_id, child_id)`, in the order the births happened.
    pub births_list: Vec<(u64, u64)>,
    pub clades_founded: Vec<u32>,
    /// Clades whose last member died, as they were at that moment.
    pub clades_extinct: Vec<Clade>,
    /// Every organism that died, as it was at that moment, in the order the deaths happened.
    pub deaths_list: Vec<(Organism, DeathCause)>,
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
    epoch_boundary(world, rules, &rng, &mut report);
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

// ---------------------------------------------------------------------------------------
// The epoch boundary (spec §13).
// ---------------------------------------------------------------------------------------

fn epoch_boundary(world: &mut World, rules: &Ruleset, rng: &Rng, report: &mut EpochReport) {
    // 1. Rift phase changes.
    rift_changes(world, rules, report);
    // 2. Natural events.
    let global_tick = world.epoch * u64::from(rules.ticks_per_epoch);
    match climate::season(rules, global_tick) {
        SPRING => flood(world, rules, rng, report),
        SUMMER => {
            wildfire(world, rules, rng, report);
            drought(world, rules, rng, report);
        }
        _ => {}
    }
    plague(world, rules, rng, report);
    // 3. Miracles arrive in stage B′.
    // 4. Natural revival.
    natural_revival(world, rules, rng, report);
}

/// Moves rift cells along their schedule (spec §10).
fn rift_changes(world: &mut World, rules: &Ruleset, report: &mut EpochReport) {
    let epoch = world.epoch;
    let mut sank = false;
    for k in 0..world.rifts.len() {
        let rift = world.rifts[k];
        let phase = rift.phase_at(epoch);
        let cell = &mut world.cells[usize::from(rift.cell)];
        if phase <= cell.rift {
            continue;
        }
        cell.rift = phase;
        report.rift_changes += 1;
        match phase {
            RiftPhase::Shallows if cell.biome.is_land() => {
                cell.biome = Biome::Shallows;
                cell.food = 0;
            }
            RiftPhase::Deep => {
                cell.biome = Biome::DeepWater;
                cell.food = 0;
                cell.detritus = 0;
                cell.moisture = 100;
                sank = true;
                if rift.bridge > 0 && !report.bridges_closed.contains(&rift.bridge) {
                    report.bridges_closed.push(rift.bridge);
                }
            }
            _ => {}
        }
    }
    if sank {
        rescue(world, rules, report);
    }
}

/// Carries organisms off cells that sank to the nearest free land: the closest by straight
/// distance within `rescue_radius`, ties broken by row, then by column. Organisms move in
/// order of id. With no room within reach they drown.
fn rescue(world: &mut World, rules: &Ruleset, report: &mut EpochReport) {
    let mut occupancy = vec![0u8; world.cells.len()];
    for o in &world.organisms {
        occupancy[usize::from(o.cell)] += 1;
    }
    let reach = i32::from(rules.rifts.rescue_radius);
    let mut offsets: Vec<(i32, i32)> = (-reach..=reach)
        .flat_map(|dy| (-reach..=reach).map(move |dx| (dx, dy)))
        .filter(|&offset| offset != (0, 0))
        .collect();
    offsets.sort_by_key(|&(dx, dy)| (dx * dx + dy * dy, dy, dx));
    let mut drowned = vec![false; world.organisms.len()];
    let mut any = false;
    for (i, drowns) in drowned.iter_mut().enumerate() {
        let o = world.organisms[i];
        let from = usize::from(o.cell);
        if rules.biomes[world.cells[from].biome as usize].passable {
            continue;
        }
        occupancy[from] -= 1;
        let (x, y) = world.coords(from);
        let site = offsets.iter().find_map(|&(dx, dy)| {
            let c = world.index(x + dx, y + dy)?;
            (world.cells[c].biome.is_land() && occupancy[c] < rules.max_per_cell).then_some(c)
        });
        if let Some(c) = site {
            world.organisms[i].cell = c as u16;
            occupancy[c] += 1;
            report.rescued += 1;
        } else {
            *drowns = true;
            any = true;
            world
                .clades
                .get_mut(&o.clade_id)
                .expect("every organism has a clade")
                .living -= 1;
            report.deaths_drowned += 1;
        }
    }
    if any {
        let mut k = 0;
        world.organisms.retain(|_| {
            let keep = !drowned[k];
            k += 1;
            keep
        });
        close_extinct_clades(world, rules, report);
    }
}

fn active(world: &World, kind: EffectKind) -> u32 {
    world.effects.iter().filter(|e| e.kind == kind).count() as u32
}

/// The cells an effect covers, as a mask.
fn mask(world: &World, effect: &Effect) -> Vec<bool> {
    (0..world.cells.len())
        .map(|i| effect.covers(world, i))
        .collect()
}

fn next_to_water(world: &World, cell: usize) -> bool {
    let (x, y) = world.coords(cell);
    NEIGHBORS.iter().any(|&(dx, dy)| {
        world
            .index(x + dx, y + dy)
            .is_some_and(|c| !world.cells[c].biome.is_land())
    })
}

/// A spring flood around a swamp or a cell next to water. Covered land loses its food and is
/// soaked; while the flood lasts it acts as shallows.
fn flood(world: &mut World, rules: &Ruleset, rng: &Rng, report: &mut EpochReport) {
    let ev = &rules.events;
    let subject = world.epoch;
    if active(world, EffectKind::Flood) >= ev.max_active_per_kind
        || !rng.chance_ppm(0, Purpose::FloodChance, subject, ev.flood_ppm)
    {
        return;
    }
    let sites: Vec<usize> = (0..world.cells.len())
        .filter(|&i| {
            let biome = world.cells[i].biome;
            biome.floods() && (biome == Biome::Swamp || next_to_water(world, i))
        })
        .collect();
    if sites.is_empty() {
        return;
    }
    let center = sites[rng.below(0, Purpose::FloodSite, subject, sites.len() as u64) as usize];
    let flood = Effect {
        kind: EffectKind::Flood,
        center: center as u16,
        radius: ev.flood_radius,
        remaining_ticks: ev.flood_ticks,
    };
    let covered = mask(world, &flood);
    for (cell, &hit) in world.cells.iter_mut().zip(&covered) {
        if hit {
            cell.food = 0;
            cell.moisture = 100;
        }
    }
    if ev.flood_ticks > 0 {
        world.effects.push(flood);
    }
    report.floods += 1;
}

fn wildfire(world: &mut World, rules: &Ruleset, rng: &Rng, report: &mut EpochReport) {
    let ev = &rules.events;
    let subject = world.epoch;
    if active(world, EffectKind::Ash) >= ev.max_active_per_kind
        || !rng.chance_ppm(0, Purpose::WildfireChance, subject, ev.wildfire_ppm)
    {
        return;
    }
    let sites: Vec<usize> = (0..world.cells.len())
        .filter(|&i| {
            let c = world.cells[i];
            matches!(c.biome, Biome::Forest | Biome::Steppe)
                && c.moisture < ev.wildfire_max_moisture
        })
        .collect();
    if sites.is_empty() {
        return;
    }
    let center = sites[rng.below(0, Purpose::WildfireSite, subject, sites.len() as u64) as usize];
    let span = u64::from(ev.wildfire_radius_max - ev.wildfire_radius_min) + 1;
    let radius =
        ev.wildfire_radius_min + rng.below(0, Purpose::WildfireRadius, subject, span) as u8;
    let fire = Effect {
        kind: EffectKind::Ash,
        center: center as u16,
        radius,
        remaining_ticks: ev.ash_ticks,
    };
    let burned = mask(world, &fire);
    for (cell, &hit) in world.cells.iter_mut().zip(&burned) {
        if hit {
            cell.food = 0;
            cell.detritus = 0;
        }
    }
    for o in &mut world.organisms {
        if burned[usize::from(o.cell)] {
            o.energy = (o.energy * (100 - ev.wildfire_energy_loss_pct) / 100).max(1);
        }
    }
    if ev.ash_ticks > 0 {
        world.effects.push(fire);
    }
    report.wildfires += 1;
}

fn drought(world: &mut World, rules: &Ruleset, rng: &Rng, report: &mut EpochReport) {
    let ev = &rules.events;
    let subject = world.epoch;
    if active(world, EffectKind::Drought) >= ev.max_active_per_kind
        || !rng.chance_ppm(0, Purpose::DroughtChance, subject, ev.drought_ppm)
    {
        return;
    }
    let sites: Vec<usize> = (0..world.cells.len())
        .filter(|&i| matches!(world.cells[i].biome, Biome::Steppe | Biome::Desert))
        .collect();
    if sites.is_empty() {
        return;
    }
    let center = sites[rng.below(0, Purpose::DroughtSite, subject, sites.len() as u64) as usize];
    let drought = Effect {
        kind: EffectKind::Drought,
        center: center as u16,
        radius: ev.drought_radius,
        remaining_ticks: ev.drought_ticks,
    };
    let dried = mask(world, &drought);
    for (cell, &hit) in world.cells.iter_mut().zip(&dried) {
        if hit {
            cell.moisture = cell.moisture.saturating_sub(ev.drought_moisture_drop);
        }
    }
    if ev.drought_ticks > 0 {
        world.effects.push(drought);
    }
    report.droughts += 1;
}

/// "Kill the winner": the more the largest clade dominates, the likelier a plague strikes it.
fn plague(world: &mut World, rules: &Ruleset, rng: &Rng, report: &mut EpochReport) {
    let ev = &rules.events;
    let population = world.organisms.len() as u64;
    if population == 0 {
        return;
    }
    let Some(dominant) = world
        .clades
        .values()
        .max_by_key(|c| (c.living, Reverse(c.id)))
        .copied()
    else {
        return;
    };
    let share = u64::from(dominant.living) * 1000 / population;
    let min = u64::from(ev.plague_min_share_permille);
    if share <= min {
        return;
    }
    let ppm = (u64::from(ev.plague_max_ppm) * (share - min) / (1000 - min)) as u32;
    let subject = world.epoch;
    if !rng.chance_ppm(0, Purpose::PlagueChance, subject, ppm) {
        return;
    }
    let members: Vec<usize> = (0..world.organisms.len())
        .filter(|&i| world.organisms[i].clade_id == dominant.id)
        .collect();
    let pick = rng.below(0, Purpose::PlagueSite, subject, members.len() as u64) as usize;
    let (cx, cy) = world.coords(usize::from(world.organisms[members[pick]].cell));
    let r = i32::from(ev.plague_radius);
    let reached: Vec<usize> = members
        .into_iter()
        .filter(|&i| {
            let (x, y) = world.coords(usize::from(world.organisms[i].cell));
            (x - cx) * (x - cx) + (y - cy) * (y - cy) <= r * r
        })
        .collect();
    if (reached.len() as u32) < ev.plague_min_members {
        return;
    }
    report.plagues += 1;
    let mut dead = vec![false; world.organisms.len()];
    for i in reached {
        let o = world.organisms[i];
        if rng.chance_ppm(0, Purpose::PlagueDeath, o.id, ev.plague_mortality_ppm) {
            dead[i] = true;
            world
                .clades
                .get_mut(&o.clade_id)
                .expect("every organism has a clade")
                .living -= 1;
            let cell = &mut world.cells[usize::from(o.cell)];
            cell.detritus = cell.detritus.saturating_add(rules.body_detritus);
            report.deaths_plague += 1;
        }
    }
    let mut k = 0;
    world.organisms.retain(|_| {
        let keep = !dead[k];
        k += 1;
        keep
    });
    close_extinct_clades(world, rules, report);
}

/// The spore bank (spec §12). When fewer than `revival.below` organisms are alive, it places
/// `per_genome` organisms of each of its genomes in free cells of that genome's biome, or of
/// any land if the biome has no room, at most once per `cooldown_epochs`. Each genome founds a
/// new clade. `season_end_count` revivals within `season_end_days` end the season.
fn natural_revival(world: &mut World, rules: &Ruleset, rng: &Rng, report: &mut EpochReport) {
    let rv = &rules.revival;
    let epoch = world.epoch;
    if world.ended
        || world.spore_bank.is_empty()
        || rv.per_genome == 0
        || world.organisms.len() as u64 >= u64::from(rv.below)
        || world
            .revivals
            .last()
            .is_some_and(|&last| epoch < last + u64::from(rv.cooldown_epochs))
    {
        return;
    }
    let mut occupancy = vec![0u8; world.cells.len()];
    for o in &world.organisms {
        occupancy[usize::from(o.cell)] += 1;
    }
    for k in 0..world.spore_bank.len() {
        let spore = world.spore_bank[k];
        let free = |c: usize, biome_matches: bool| {
            let biome = world.cells[c].biome;
            occupancy[c] < rules.max_per_cell
                && biome.is_land()
                && (!biome_matches || biome == spore.biome)
        };
        let mut sites: Vec<usize> = (0..world.cells.len()).filter(|&c| free(c, true)).collect();
        if sites.is_empty() {
            sites = (0..world.cells.len()).filter(|&c| free(c, false)).collect();
        }
        let clade_id = world.next_clade_id;
        let mut placed = 0u32;
        for _ in 0..rv.per_genome {
            if sites.is_empty() || world.organisms.len() >= rules.max_organisms as usize {
                break;
            }
            let id = world.next_organism_id;
            let pick = rng.below(0, Purpose::RevivalSite, id, sites.len() as u64) as usize;
            let cell = sites[pick];
            occupancy[cell] += 1;
            if occupancy[cell] >= rules.max_per_cell {
                sites.remove(pick);
            }
            world.next_organism_id = id.checked_add(1).expect("organism id overflow");
            world.organisms.push(Organism {
                id,
                parent_id: 0,
                lineage_id: k as u32 + 1,
                clade_id,
                cell: cell as u16,
                age: 0,
                energy: rules.genesis_energy,
                genome: spore.genome,
            });
            placed += 1;
        }
        if placed > 0 {
            world.next_clade_id = clade_id.checked_add(1).expect("clade id overflow");
            world.clades.insert(
                clade_id,
                Clade {
                    id: clade_id,
                    parent_id: 0,
                    reference: spore.genome,
                    founded_epoch: epoch,
                    living: placed,
                    peak_living: placed,
                },
            );
            report.clades_founded.push(clade_id);
        }
        report.revived += placed;
    }
    world.revivals.push(epoch);
    report.revival = true;
    let window = u64::from(rv.season_end_days) * u64::from(rules.epochs_per_day);
    let recent = world
        .revivals
        .iter()
        .filter(|&&e| e + window > epoch)
        .count();
    if recent as u64 >= u64::from(rv.season_end_count) {
        world.ended = true;
    }
}

/// Removes clades whose last member died. A named clade, one that once had
/// `clade_name_threshold` members alive at the same time, goes to the museum; when the museum
/// is full, the oldest entry leaves it (spec §12).
fn close_extinct_clades(world: &mut World, rules: &Ruleset, report: &mut EpochReport) {
    let extinct: Vec<u32> = world
        .clades
        .values()
        .filter(|c| c.living == 0)
        .map(|c| c.id)
        .collect();
    for id in extinct {
        let Some(clade) = world.clades.remove(&id) else {
            continue;
        };
        if clade.peak_living >= rules.clade_name_threshold && rules.museum_capacity > 0 {
            if world.museum.len() >= rules.museum_capacity as usize {
                world.museum.remove(0);
            }
            world.museum.push(MuseumEntry {
                clade_id: clade.id,
                parent_id: clade.parent_id,
                reference: clade.reference,
                founded_epoch: clade.founded_epoch,
                extinct_epoch: world.epoch,
                peak_living: clade.peak_living,
            });
        }
        report.clades_extinct.push(clade);
    }
}

// ---------------------------------------------------------------------------------------
// Ticks.
// ---------------------------------------------------------------------------------------

/// Working memory of one tick. Not part of the state.
#[derive(Default)]
struct Scratch {
    /// Organisms per cell.
    occ: Vec<u8>,
    /// Indexes of the organisms in each cell, in arrival order.
    members: Vec<[u32; 4]>,
    /// The strongest hunter in each cell at the start of the tick: (attack, clade).
    hunter: Vec<(i32, u32)>,
    /// The weakest organism in each cell at the start of the tick: (defense with cover, clade).
    prey: Vec<(i32, u32)>,
    /// (queue key, organism id, organism index).
    queue: Vec<(u64, u64, u32)>,
    alive: Vec<bool>,
    /// Energy spent on movement and attacks during the tick.
    spent: Vec<i32>,
    alive_count: usize,
    /// Food growth multiplier from active effects, per cell, in percent.
    effect_pct: Vec<u64>,
    /// The biome each cell acts as this tick: flooded land acts as shallows.
    biome: Vec<Biome>,
    /// The energy cost of stepping into each cell this tick.
    move_cost: Vec<i32>,
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
    // 1. Environment.
    environment(world, rules, tick, s);

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
        let defense = o.genome.defense(rules) + cover(rules, s.biome[c]);
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

    // 4. Energy costs; deaths from starvation and old age. Cold raises the cost in the north
    // in winter.
    let global_tick = world.epoch * u64::from(rules.ticks_per_epoch) + u64::from(tick);
    let cold: Vec<i64> = if rules.climate.cold_winter_pct == 0 {
        Vec::new()
    } else {
        (0..i64::from(world.height))
            .map(|row| climate::cold_pct(rules, row, global_tick))
            .collect()
    };
    for i in 0..world.organisms.len() {
        if !s.alive[i] {
            continue;
        }
        let o = world.organisms[i];
        let biome = s.biome[usize::from(o.cell)];
        let mut cost = o.genome.upkeep(rules);
        if let Some(preferred) = Biome::from_habitat(o.genome.habitat) {
            let delta = cost * rules.habitat_modifier_pct / 100;
            cost = if biome == preferred {
                cost - delta
            } else {
                cost + delta
            };
        }
        if !cold.is_empty() {
            let row = usize::from(o.cell) / usize::from(world.width);
            cost += (i64::from(cost) * cold[row] / 100) as i32;
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
    close_extinct_clades(world, rules, report);
}

/// Step 1 of a tick: moisture, decomposition and food growth under the time of year, the
/// active effects and rift faults; then the effects count down (spec §10). It also notes how
/// each cell acts this tick: flooded land acts as shallows, and faults cost more to enter.
fn environment(world: &mut World, rules: &Ruleset, tick: u32, s: &mut Scratch) {
    let global_tick = world.epoch * u64::from(rules.ticks_per_epoch) + u64::from(tick);
    let growth = Biome::ALL.map(|b| climate::growth_pct(rules, b, global_tick));
    let target = Biome::ALL.map(|b| climate::target_moisture(rules, b, global_tick));

    s.effect_pct.clear();
    s.effect_pct.resize(world.cells.len(), 100);
    s.biome.clear();
    s.biome.extend(world.cells.iter().map(|c| c.biome));
    for effect in &world.effects {
        let (cx, cy) = world.coords(usize::from(effect.center));
        let r = i32::from(effect.radius);
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                let Some(c) = world.index(x, y) else {
                    continue;
                };
                if !effect.covers(world, c) {
                    continue;
                }
                match effect.kind {
                    EffectKind::Ash => {
                        s.effect_pct[c] =
                            s.effect_pct[c] * u64::from(rules.events.ash_growth_pct) / 100;
                    }
                    EffectKind::Drought => {
                        s.effect_pct[c] =
                            s.effect_pct[c] * u64::from(rules.events.drought_growth_pct) / 100;
                    }
                    EffectKind::Flood => s.biome[c] = Biome::Shallows,
                }
            }
        }
    }

    let relax = rules.climate.moisture_relax;
    let fault_growth = u64::from(rules.rifts.fault_growth_pct);
    s.move_cost.clear();
    for (i, cell) in world.cells.iter_mut().enumerate() {
        let acting = s.biome[i];
        let faulted = cell.rift == RiftPhase::Fault && cell.biome.is_land() && acting == cell.biome;
        let step = rules.biomes[acting as usize].move_cost;
        s.move_cost.push(if faulted {
            step * rules.rifts.fault_move_pct / 100
        } else {
            step
        });
        let p = rules.biomes[cell.biome as usize];
        if !p.passable || acting != cell.biome {
            // Deep water, or flooded land: nothing grows and the soil stays soaked.
            continue;
        }
        let t = target[cell.biome as usize];
        cell.moisture = if cell.moisture < t {
            cell.moisture.saturating_add(relax).min(t)
        } else {
            cell.moisture.saturating_sub(relax).max(t)
        };
        let decomposed =
            (u64::from(cell.detritus) * u64::from(rules.decomposition_pct) / 100) as u32;
        cell.detritus -= decomposed;
        let mut regen = u64::from(p.base_regen)
            * growth[cell.biome as usize]
            * climate::moisture_pct(rules, cell.moisture)
            * s.effect_pct[i]
            / 1_000_000;
        if faulted {
            regen = regen * fault_growth / 100;
        }
        let regen = u32::try_from(regen).unwrap_or(u32::MAX);
        cell.food = cell
            .food
            .saturating_add(decomposed)
            .saturating_add(regen)
            .min(p.food_max);
    }

    world.effects.retain_mut(|e| {
        e.remaining_ticks -= 1;
        e.remaining_ticks > 0
    });
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
                + cover(rules, s.biome[usize::from(prey.cell)])
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
            let acting = s.biome[c];
            let p = rules.biomes[acting as usize];
            if !p.passable || (c != here && s.occ[c] >= rules.max_per_cell) {
                continue;
            }
            let food = world.cells[c].food;
            let others = i64::from(s.occ[c]) - i64::from(c == here);
            let distance = i64::from(dx.abs().max(dy.abs()));

            let mut score = 0i64;
            if bite > 0 {
                score += i64::from(food.min(bite))
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
            if preferred == Some(acting) {
                score += i64::from(w.habitat_bonus);
            }
            score -= others * i64::from(w.crowd_per_neighbor);
            score -= distance * i64::from(s.move_cost[c]);
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

/// Hunters hunt only while hungry (satiation, spec §11.3), which keeps them from wiping out
/// their prey.
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
            if rules.biomes[s.biome[n] as usize].passable && s.occ[n] < rules.max_per_cell {
                next = Some(n);
                break;
            }
        }
        let Some(n) = next else {
            break;
        };
        s.remove(pos, i as u32);
        s.add(n, i as u32);
        s.spent[i] += s.move_cost[n];
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
                let margin = my_attack - (other.genome.defense(rules) + cover(rules, s.biome[n]));
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
        DeathCause::Starvation | DeathCause::OldAge | DeathCause::Plague => rules.body_detritus,
        DeathCause::Drowned => 0,
    };
    world.cells[cell].detritus = world.cells[cell].detritus.saturating_add(detritus);
    report.deaths_list.push((o, cause));
    match cause {
        DeathCause::Starvation => report.deaths_starvation += 1,
        DeathCause::OldAge => report.deaths_old_age += 1,
        DeathCause::Predation => report.deaths_predation += 1,
        DeathCause::Plague => report.deaths_plague += 1,
        DeathCause::Drowned => report.deaths_drowned += 1,
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
                    if rules.biomes[s.biome[c] as usize].passable && s.occ[c] < rules.max_per_cell {
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
