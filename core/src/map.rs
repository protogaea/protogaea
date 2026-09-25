//! Terrain generation and genesis.
//!
//! A single continent surrounded by water, with coastal shallows, and the rift plan that will
//! break it apart (spec §4).

use std::cmp::Reverse;
use std::collections::BTreeMap;

use crate::climate;
use crate::rifts::{self, Plan};
use crate::rng::{derive, Purpose, Rng};
use crate::ruleset::{BiomeMix, Ruleset};
use crate::state::{Biome, Cell, Clade, Organism, RiftPhase, World};

fn genesis_rng(genesis_seed: &[u8; 32]) -> Rng {
    Rng::new(&derive(b"PROTOGAEA/GENESIS/V0", &[genesis_seed]))
}

/// The terrain at genesis and the rift plan, both drawn from the genesis seed. The plan is
/// published before a season starts (spec §4); its schedule becomes part of the state.
pub fn world_plan(rules: &Ruleset, genesis_seed: &[u8; 32]) -> (Vec<Biome>, Plan) {
    let rng = genesis_rng(genesis_seed);
    let biomes = generate_terrain(rules, &rng);
    let plan = rifts::plan(rules, &rng, &biomes);
    (biomes, plan)
}

/// Creates the world at epoch 0.
pub fn genesis(rules: &Ruleset, genesis_seed: &[u8; 32], world_id: [u8; 16]) -> World {
    let rng = genesis_rng(genesis_seed);
    let (biomes, plan) = world_plan(rules, genesis_seed);
    let mut cells: Vec<Cell> = biomes
        .iter()
        .map(|&biome| Cell {
            biome,
            food: rules.biomes[biome as usize].food_max,
            detritus: 0,
            moisture: climate::target_moisture(rules, biome, 0),
            rift: RiftPhase::None,
        })
        .collect();
    for r in &plan.rifts {
        cells[usize::from(r.cell)].rift = RiftPhase::Crack;
    }
    let mut world = World {
        world_id,
        ruleset_id: rules.ruleset_id(),
        width: rules.width,
        height: rules.height,
        epoch: 0,
        cells,
        organisms: Vec::new(),
        clades: BTreeMap::new(),
        effects: Vec::new(),
        rifts: plan.rifts,
        museum: Vec::new(),
        spore_bank: rules.founders.clone(),
        revivals: Vec::new(),
        ended: false,
        next_organism_id: 1,
        next_clade_id: 1,
    };
    let per_cell_at_genesis = rules.max_per_cell.min(2);
    let mut occupancy = vec![0u8; world.cells.len()];
    for (lineage, founder) in (0u32..).zip(&rules.founders) {
        let preferred = founder.biome;
        let founder = founder.genome;
        let mut sites: Vec<usize> = (0..world.cells.len())
            .filter(|&i| world.cells[i].biome == preferred)
            .collect();
        if sites.is_empty() {
            sites = (0..world.cells.len())
                .filter(|&i| world.cells[i].biome.is_land())
                .collect();
        }
        if sites.is_empty() {
            break;
        }
        let pick = rng.below(
            0,
            Purpose::GenesisSite,
            u64::from(lineage),
            sites.len() as u64,
        );
        let (sx, sy) = world.coords(sites[pick as usize]);
        let clade_id = world.next_clade_id;
        world.next_clade_id += 1;
        let mut placed = 0u32;
        // Fill rings around the site, at most two founders per cell.
        'rings: for radius in 0..=16i32 {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs().max(dy.abs()) != radius {
                        continue;
                    }
                    let Some(c) = world.index(sx + dx, sy + dy) else {
                        continue;
                    };
                    if !world.cells[c].biome.is_land() {
                        continue;
                    }
                    while occupancy[c] < per_cell_at_genesis && placed < rules.organisms_per_lineage
                    {
                        let id = world.next_organism_id;
                        world.next_organism_id += 1;
                        let age = rng.below(
                            0,
                            Purpose::GenesisAge,
                            id,
                            u64::from(rules.senescence_start),
                        );
                        world.organisms.push(Organism {
                            id,
                            parent_id: 0,
                            lineage_id: lineage + 1,
                            clade_id,
                            cell: c as u16,
                            age: age as u32,
                            energy: rules.genesis_energy,
                            genome: founder,
                        });
                        occupancy[c] += 1;
                        placed += 1;
                    }
                    if placed == rules.organisms_per_lineage {
                        break 'rings;
                    }
                }
            }
        }
        if placed > 0 {
            world.clades.insert(
                clade_id,
                Clade {
                    id: clade_id,
                    parent_id: 0,
                    reference: founder,
                    founded_epoch: 0,
                    living: placed,
                    peak_living: placed,
                },
            );
        }
    }
    world
}

/// Generates one continent: value noise for height and moisture, a radial falloff towards
/// the edges, then biomes by percentiles so that the proportions are stable across seeds;
/// with `Rifts::plate_mixes`, separately on each plate.
pub fn generate_terrain(rules: &Ruleset, rng: &Rng) -> Vec<Biome> {
    let (w, h) = (i64::from(rules.width), i64::from(rules.height));
    let n = (w * h) as usize;
    let height = value_noise(rules, rng, Purpose::MapHeight, &[(16, 4), (8, 2), (4, 1)]);
    let moisture = value_noise(rules, rng, Purpose::MapMoisture, &[(16, 2), (8, 1)]);

    // A continent: height minus a radial falloff that sinks the edges.
    let (cx, cy) = (w / 2, h / 2);
    let radius2 = (cx * cx).max(1);
    let adjusted: Vec<i64> = (0..n)
        .map(|i| {
            let (x, y) = (i as i64 % w, i as i64 / w);
            let d2 = (x - cx) * (x - cx) + (y - cy) * (y - cy);
            height[i] - d2 * 7000 / radius2
        })
        .collect();

    // Land: the highest land_share_pct of cells by adjusted height.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (Reverse(adjusted[i]), i));
    let land_count = n * rules.land_share_pct as usize / 100;
    let mut biomes = vec![Biome::DeepWater; n];
    let mut is_land = vec![false; n];
    for &i in &order[..land_count] {
        is_land[i] = true;
    }

    // One biome mix for all the land, or one per plate (a future continent): the plates take
    // the mixes in turn from a random starting point.
    let mixes = &rules.rifts.plate_mixes;
    let groups: Vec<(BiomeMix, Vec<usize>)> = if mixes.is_empty() {
        vec![(rules.biome_mix, (0..n).filter(|&i| is_land[i]).collect())]
    } else {
        let (plates, plate_count) = rifts::plate_map(rules, rng, &is_land);
        let first = rng.below(0, Purpose::PlateMix, 0, mixes.len() as u64) as usize;
        (0..plate_count)
            .map(|plate| {
                let mix = mixes[(first + usize::from(plate)) % mixes.len()];
                let land = (0..n)
                    .filter(|&i| is_land[i] && plates[i] == plate)
                    .collect();
                (mix, land)
            })
            .collect()
    };
    for (mix, mut land) in groups {
        // Mountains: the highest land by raw height.
        land.sort_by_key(|&i| (Reverse(height[i]), i));
        let mountains = land.len() * usize::from(mix.mountains_pct) / 100;
        for &i in &land[..mountains] {
            biomes[i] = Biome::Mountains;
        }

        // The rest of the land by moisture, from dry to wet.
        let mut rest = land[mountains..].to_vec();
        rest.sort_by_key(|&i| (moisture[i], i));
        let total = rest.len().max(1);
        let desert = usize::from(mix.desert_pct);
        let steppe = desert + usize::from(mix.steppe_pct);
        let forest = steppe + usize::from(mix.forest_pct);
        for (k, &i) in rest.iter().enumerate() {
            let pct = k * 100 / total;
            biomes[i] = if pct < desert {
                Biome::Desert
            } else if pct < steppe {
                Biome::Steppe
            } else if pct < forest {
                Biome::Forest
            } else {
                Biome::Swamp
            };
        }
    }

    // Shallows: water next to land.
    let with_land = biomes.clone();
    for i in 0..n {
        if with_land[i] != Biome::DeepWater {
            continue;
        }
        let (x, y) = (i as i64 % w, i as i64 / w);
        let coastal = (-1..=1).any(|dy| {
            (-1..=1).any(|dx| {
                let (nx, ny) = (x + dx, y + dy);
                nx >= 0
                    && ny >= 0
                    && nx < w
                    && ny < h
                    && with_land[(ny * w + nx) as usize].is_land()
            })
        });
        if coastal {
            biomes[i] = Biome::Shallows;
        }
    }
    biomes
}

/// Integer value noise: for each octave `(cell_size, amplitude)`, random lattice values in
/// `0..1024` interpolated bilinearly.
pub(crate) fn value_noise(
    rules: &Ruleset,
    rng: &Rng,
    purpose: Purpose,
    octaves: &[(i64, i64)],
) -> Vec<i64> {
    let (w, h) = (i64::from(rules.width), i64::from(rules.height));
    let mut out = vec![0i64; (w * h) as usize];
    for (octave, &(size, amplitude)) in octaves.iter().enumerate() {
        let lattice = |gx: i64, gy: i64| -> i64 {
            let subject = ((octave as u64) << 48) | ((gx as u64) << 24) | gy as u64;
            (rng.raw(0, purpose, subject, 0) % 1024) as i64
        };
        for y in 0..h {
            for x in 0..w {
                let (gx, gy, fx, fy) = (x / size, y / size, x % size, y % size);
                let v = lattice(gx, gy) * (size - fx) * (size - fy)
                    + lattice(gx + 1, gy) * fx * (size - fy)
                    + lattice(gx, gy + 1) * (size - fx) * fy
                    + lattice(gx + 1, gy + 1) * fx * fy;
                out[(y * w + x) as usize] += amplitude * v / (size * size);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn founders_start_on_land() {
        let rules = Ruleset::default();
        for founder in rules.founders {
            assert!(founder.genome.is_valid(rules.trait_budget), "{founder:?}");
            assert!(founder.biome.is_land());
        }
    }

    #[test]
    fn terrain_has_the_requested_land_share() {
        let rules = Ruleset::default();
        let biomes = generate_terrain(&rules, &Rng::new(&[7; 32]));
        let land = biomes.iter().filter(|b| b.is_land()).count();
        assert_eq!(land, biomes.len() * rules.land_share_pct as usize / 100);
        for biome in [
            Biome::Forest,
            Biome::Steppe,
            Biome::Desert,
            Biome::Mountains,
            Biome::Swamp,
            Biome::Shallows,
        ] {
            assert!(biomes.contains(&biome), "missing {biome:?}");
        }
    }

    #[test]
    fn every_plate_gets_every_biome_of_its_mix() {
        let mix = |mountains_pct, desert_pct, steppe_pct, forest_pct| BiomeMix {
            mountains_pct,
            desert_pct,
            steppe_pct,
            forest_pct,
        };
        let mut rules = Ruleset::default();
        rules.rifts.plate_mixes = vec![mix(10, 40, 40, 10), mix(30, 5, 15, 60)];
        rules.validate().unwrap();
        for seed in 1..6u8 {
            let rng = Rng::new(&[seed; 32]);
            let biomes = generate_terrain(&rules, &rng);
            let is_land: Vec<bool> = biomes.iter().map(|b| b.is_land()).collect();
            let (plates, count) = rifts::plate_map(&rules, &rng, &is_land);
            let mut mountain_shares = Vec::new();
            for plate in 0..count {
                let on_plate = |b: Biome| {
                    (0..biomes.len())
                        .filter(|&i| plates[i] == plate && biomes[i] == b)
                        .count()
                };
                for b in [
                    Biome::Forest,
                    Biome::Steppe,
                    Biome::Desert,
                    Biome::Mountains,
                    Biome::Swamp,
                ] {
                    assert!(on_plate(b) > 0, "seed {seed}, plate {plate} lacks {b:?}");
                }
                let land = (0..biomes.len())
                    .filter(|&i| plates[i] == plate && is_land[i])
                    .count();
                mountain_shares.push(on_plate(Biome::Mountains) * 100 / land);
            }
            // The two mixes alternate, so the plates differ in their mountains.
            assert!(mountain_shares.iter().any(|&s| s <= 10));
            assert!(mountain_shares.iter().any(|&s| s >= 29));
        }
    }

    #[test]
    fn genesis_places_every_founder() {
        let rules = Ruleset::default();
        let world = genesis(&rules, &[1; 32], [2; 16]);
        world.check_invariants(&rules).unwrap();
        assert_eq!(
            world.organisms.len() as u32,
            rules.founders.len() as u32 * rules.organisms_per_lineage
        );
        assert_eq!(world.clades.len(), rules.founders.len());
    }
}
