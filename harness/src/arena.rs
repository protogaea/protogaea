//! The archetype arena (spec §28): the three archetypes of §11.3 in pairs, with mutation off so
//! that each lineage stays what it is. The intended cycle is that grazers beat armored organisms
//! in the competition for food, armored organisms beat hunters (who cannot eat them and starve),
//! and hunters beat grazers.

use protogaea_core::{genesis, Ruleset, World};

use crate::runner::{genesis_inputs, Run};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Archetype {
    Grazer,
    Armored,
    Hunter,
}

impl Archetype {
    /// The founder that stands for it in the default ruleset.
    fn founder_index(self) -> usize {
        match self {
            Archetype::Grazer => 0,
            Archetype::Hunter => 1,
            Archetype::Armored => 2,
        }
    }
}

/// The rules of one arena world: only these founders, no rifts, no mutation.
pub fn arena_rules(base: &Ruleset, members: &[Archetype], per_lineage: u32) -> Ruleset {
    let mut rules = base.clone();
    rules.founders = members
        .iter()
        .map(|a| base.founders[a.founder_index()])
        .collect();
    rules.organisms_per_lineage = per_lineage;
    rules.rifts.plates_min = 0;
    rules.rifts.plates_max = 0;
    rules.rifts.plate_mixes.clear();
    rules.mutation_ppm = 0;
    rules.behavior_mutation_ppm = 0;
    // No revival from the spore bank: a lineage that dies out stays dead.
    rules.revival.below = 0;
    rules
}

/// A world where the later lineages start right next to the first one. Genesis puts every
/// lineage in its own biome, sometimes far apart; the arena tests how the archetypes fare
/// against each other, not whether they meet.
fn side_by_side(seed: u64, rules: Ruleset) -> Run {
    let (genesis_seed, world_id) = genesis_inputs(seed);
    let mut world = genesis(&rules, &genesis_seed, world_id);
    let first: Vec<usize> = world
        .organisms
        .iter()
        .filter(|o| o.lineage_id == 1)
        .map(|o| usize::from(o.cell))
        .collect();
    if first.is_empty() {
        return Run::resume(seed, rules, world);
    }
    let (sx, sy) = first.iter().fold((0i64, 0i64), |(x, y), &c| {
        let (cx, cy) = world.coords(c);
        (x + i64::from(cx), y + i64::from(cy))
    });
    let n = first.len() as i64;
    // Next to the first lineage: six cells east of its center.
    let (tx, ty) = (sx / n + 6, sy / n);
    let mut occupancy = vec![0u8; world.cells.len()];
    for o in &world.organisms {
        if o.lineage_id == 1 {
            occupancy[usize::from(o.cell)] = u8::MAX;
        }
    }
    let mut sites: Vec<usize> = (0..world.cells.len())
        .filter(|&c| world.cells[c].biome.is_land() && occupancy[c] == 0)
        .collect();
    sites.sort_by_key(|&c| {
        let (x, y) = world.coords(c);
        let (dx, dy) = (i64::from(x) - tx, i64::from(y) - ty);
        (dx * dx + dy * dy, c)
    });
    let mut next = sites.into_iter().flat_map(|c| [c, c]);
    for o in world.organisms.iter_mut().filter(|o| o.lineage_id != 1) {
        if let Some(c) = next.next() {
            o.cell = c as u16;
        }
    }
    Run::resume(seed, rules, world)
}

/// Mean population of each lineage (by founder order) over the last `tail` epochs of a run.
pub fn run_arena(seed: u64, rules: Ruleset, epochs: u64, tail: u64) -> Vec<f64> {
    let lineages = rules.founders.len();
    let mut run = side_by_side(seed, rules);
    let mut sums = vec![0u64; lineages];
    let mut counted = 0u64;
    for e in 0..epochs {
        run.step();
        if e + tail >= epochs {
            for (k, n) in count_lineages(&run.world, lineages).into_iter().enumerate() {
                sums[k] += n;
            }
            counted += 1;
        }
        if run.world.organisms.is_empty() {
            break;
        }
    }
    sums.iter()
        .map(|&s| s as f64 / counted.max(1) as f64)
        .collect()
}

/// Prints each lineage's population, and how many of it stand in cover, every `every` epochs.
pub fn trace_arena(seed: u64, rules: Ruleset, epochs: u64, every: u64) {
    let lineages = rules.founders.len();
    let mut run = side_by_side(seed, rules);
    for e in 1..=epochs {
        run.step();
        if e % every == 0 || run.world.organisms.is_empty() {
            let counts = count_lineages(&run.world, lineages);
            let mut in_cover = vec![0u64; lineages];
            for o in &run.world.organisms {
                let biome = run.world.cells[usize::from(o.cell)].biome;
                if run.rules.biomes[biome as usize].cover > 0 {
                    in_cover[o.lineage_id as usize - 1] += 1;
                }
            }
            let parts: Vec<String> = counts
                .iter()
                .zip(&in_cover)
                .map(|(n, c)| format!("{n:>5} ({c:>4} in cover)"))
                .collect();
            println!(
                "  day {:>5.2}: {}",
                e as f64 / f64::from(run.rules.epochs_per_day),
                parts.join("  ")
            );
        }
        if run.world.organisms.is_empty() {
            break;
        }
    }
}

fn count_lineages(world: &World, lineages: usize) -> Vec<u64> {
    let mut counts = vec![0u64; lineages];
    for o in &world.organisms {
        if let Some(c) = counts.get_mut(o.lineage_id as usize - 1) {
            *c += 1;
        }
    }
    counts
}
