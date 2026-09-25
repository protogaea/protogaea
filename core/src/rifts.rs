//! The Breaking of Pangea (spec §4, §10): plates, rift lines, land bridges and their schedule.
//!
//! At genesis the continent is divided into plates, the future continents: the cells nearest
//! to centers spread evenly around the middle of the land, with boundaries bent by noise.
//! Every passable cell next to a cell of another plate lies on a rift. Rift cells become
//! faults, then shallows and deep water in waves from the ocean inward. One land bridge per
//! pair of neighboring plates stays land until it closes in phase V. Once every rift cell is
//! deep water, no step leads from one plate to another.

use std::collections::{BTreeSet, VecDeque};

use crate::map::value_noise;
use crate::rng::{Purpose, Rng};
use crate::ruleset::Ruleset;
use crate::state::{Biome, Rift};

/// Unit vectors for 24 directions, 15° apart, ×1024.
const DIRECTIONS: [(i64, i64); 24] = [
    (1024, 0),
    (989, 265),
    (887, 512),
    (724, 724),
    (512, 887),
    (265, 989),
    (0, 1024),
    (-265, 989),
    (-512, 887),
    (-724, 724),
    (-887, 512),
    (-989, 265),
    (-1024, 0),
    (-989, -265),
    (-887, -512),
    (-724, -724),
    (-512, -887),
    (-265, -989),
    (0, -1024),
    (265, -989),
    (512, -887),
    (724, -724),
    (887, -512),
    (989, -265),
];

/// The eight neighbors, in a fixed order.
const NEIGHBORS: [(i64, i64); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// The largest value of the bending noise: octaves of amplitude 2 and 1 over `0..1024`.
const BEND_MAX: i64 = 3 * 1023;

/// A land bridge (spec §4, phase V).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bridge {
    /// From 1, as in `Rift::bridge`.
    pub id: u8,
    pub center: u16,
    /// The two plates it joins.
    pub plates: (u8, u8),
    pub close_epoch: u64,
}

/// The rift plan of a world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The plate of every cell. Plates are the future continents.
    pub plates: Vec<u8>,
    pub plate_count: u8,
    /// The schedule, by cell index.
    pub rifts: Vec<Rift>,
    pub bridges: Vec<Bridge>,
}

struct Grid {
    w: i64,
    h: i64,
}

impl Grid {
    fn at(&self, x: i64, y: i64) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.w && y < self.h).then(|| (y * self.w + x) as usize)
    }

    fn xy(&self, i: usize) -> (i64, i64) {
        (i as i64 % self.w, i as i64 / self.w)
    }

    fn neighbors(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        let (x, y) = self.xy(i);
        NEIGHBORS
            .into_iter()
            .filter_map(move |(dx, dy)| self.at(x + dx, y + dy))
    }
}

/// Divides the map into plates: every cell belongs to the nearest plate center, measured from
/// its position bent by noise. The centers are spread evenly around the middle of the land.
/// Returns the plate of every cell and the number of plates; without rifts, one plate.
pub fn plate_map(rules: &Ruleset, rng: &Rng, is_land: &[bool]) -> (Vec<u8>, u8) {
    let r = &rules.rifts;
    let n = is_land.len();
    let land: Vec<usize> = (0..n).filter(|&i| is_land[i]).collect();
    if r.plates_max == 0 || land.is_empty() {
        return (vec![0; n], 1);
    }
    let grid = Grid {
        w: i64::from(rules.width),
        h: i64::from(rules.height),
    };

    // Plate centers: evenly spaced around the middle of the land, halfway to its edge.
    // Positions are in sixteenths of a cell.
    let span = u64::from(r.plates_max - r.plates_min) + 1;
    let count = r.plates_min + rng.below(0, Purpose::RiftPlates, 0, span) as u8;
    let (sum_x, sum_y) = land.iter().fold((0i64, 0i64), |(sx, sy), &i| {
        let (x, y) = grid.xy(i);
        (sx + x, sy + y)
    });
    let len = land.len() as i64;
    let (cx, cy) = (sum_x * 16 / len + 8, sum_y * 16 / len + 8);
    // The radius of a disc with the land's area: area × 1024 / (π × 1024).
    let radius = (land.len() as u64 * 1024 / 3217).isqrt() as i64;
    let half = radius * 8;
    let rotation = rng.below(0, Purpose::RiftRotation, 0, 24) as usize;
    let centers: Vec<(i64, i64)> = (0..usize::from(count))
        .map(|k| {
            let (dx, dy) = DIRECTIONS[(rotation + k * 24 / usize::from(count)) % 24];
            (
                cx + (dx + 1024) * half / 1024 - half,
                cy + (dy + 1024) * half / 1024 - half,
            )
        })
        .collect();

    // Each cell belongs to the nearest center, measured from its position bent by noise.
    let bend_x = value_noise(rules, rng, Purpose::RiftWarpX, &[(16, 2), (8, 1)]);
    let bend_y = value_noise(rules, rng, Purpose::RiftWarpY, &[(16, 2), (8, 1)]);
    let warp = i64::from(r.boundary_warp) * 16;
    let plates: Vec<u8> = (0..n)
        .map(|i| {
            let (x, y) = grid.xy(i);
            let px = x * 16 + 8 + bend_x[i] * 2 * warp / BEND_MAX - warp;
            let py = y * 16 + 8 + bend_y[i] * 2 * warp / BEND_MAX - warp;
            (0..centers.len())
                .min_by_key(|&k| {
                    let (ax, ay) = centers[k];
                    ((px - ax) * (px - ax) + (py - ay) * (py - ay), k)
                })
                .expect("at least one plate") as u8
        })
        .collect();
    (plates, count)
}

/// Draws the plates, rift lines and land bridges of a world from its genesis randomness.
pub fn plan(rules: &Ruleset, rng: &Rng, biomes: &[Biome]) -> Plan {
    let r = &rules.rifts;
    let n = biomes.len();
    let is_land: Vec<bool> = biomes.iter().map(|b| b.is_land()).collect();
    let (plates, count) = plate_map(rules, rng, &is_land);
    if count == 1 {
        return Plan {
            plates,
            plate_count: 1,
            rifts: Vec::new(),
            bridges: Vec::new(),
        };
    }
    let grid = Grid {
        w: i64::from(rules.width),
        h: i64::from(rules.height),
    };

    // Rift cells: passable cells next to another plate.
    let passable = |i: usize| rules.biomes[biomes[i] as usize].passable;
    let on_rift: Vec<bool> = (0..n)
        .map(|i| passable(i) && grid.neighbors(i).any(|j| plates[j] != plates[i]))
        .collect();

    // Waves: the distance along the rift from the open ocean.
    let mut dist = vec![u32::MAX; n];
    let mut queue = VecDeque::new();
    for i in 0..n {
        if on_rift[i] && grid.neighbors(i).any(|j| biomes[j] == Biome::DeepWater) {
            dist[i] = 0;
            queue.push_back(i);
        }
    }
    while let Some(i) = queue.pop_front() {
        for j in grid.neighbors(i) {
            if on_rift[j] && dist[j] == u32::MAX {
                dist[j] = dist[i] + 1;
                queue.push_back(j);
            }
        }
    }
    let deepest = (0..n)
        .filter(|&i| on_rift[i] && dist[i] != u32::MAX)
        .map(|i| dist[i])
        .max()
        .unwrap_or(0);
    for d in dist.iter_mut() {
        if *d == u32::MAX {
            *d = deepest;
        }
    }

    // Land bridges: one per pair of plates that share a boundary on land, in the middle of
    // that boundary's land, away from other plates and the coast, and in different biomes
    // where possible.
    let mut pairs = BTreeSet::new();
    for i in 0..n {
        if on_rift[i] && biomes[i].is_land() {
            for j in grid.neighbors(i) {
                if biomes[j].is_land() && plates[j] != plates[i] {
                    pairs.insert((plates[i].min(plates[j]), plates[i].max(plates[j])));
                }
            }
        }
    }
    let reach = i64::from(r.bridge_radius);
    let clear = reach + 1;
    let mut sites: Vec<(usize, u8, u8)> = Vec::new();
    let mut used: Vec<Biome> = Vec::new();
    for (k, &(a, b)) in pairs.iter().enumerate() {
        let joins = |p: u8| p == a || p == b;
        let candidates: Vec<usize> = (0..n)
            .filter(|&i| {
                let (x, y) = grid.xy(i);
                on_rift[i]
                    && biomes[i].is_land()
                    && joins(plates[i])
                    && grid
                        .neighbors(i)
                        .any(|j| joins(plates[j]) && plates[j] != plates[i])
                    && (-clear..=clear).all(|dy| {
                        (-clear..=clear).all(|dx| {
                            grid.at(x + dx, y + dy)
                                .is_some_and(|j| biomes[j].is_land() && joins(plates[j]))
                        })
                    })
            })
            .collect();
        if candidates.is_empty() {
            continue;
        }
        let fresh: Vec<usize> = candidates
            .iter()
            .copied()
            .filter(|&i| !used.contains(&biomes[i]))
            .collect();
        let pool = if fresh.is_empty() {
            &candidates
        } else {
            &fresh
        };
        let pick = rng.below(0, Purpose::BridgeSite, k as u64, pool.len() as u64) as usize;
        let center = pool[pick];
        used.push(biomes[center]);
        sites.push((center, a, b));
    }
    let mut bridge_of = vec![0u8; n];
    for (b, &(center, _, _)) in sites.iter().enumerate() {
        let (x, y) = grid.xy(center);
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                if let Some(c) = grid.at(x + dx, y + dy) {
                    if on_rift[c] && bridge_of[c] == 0 {
                        bridge_of[c] = b as u8 + 1;
                    }
                }
            }
        }
    }

    // The schedule. Bridges close one by one, in a random order.
    let per_day = u64::from(rules.epochs_per_day);
    let day = |d: u32| u64::from(d) * per_day;
    let bridge_count = sites.len();
    let mut order: Vec<usize> = (0..bridge_count).collect();
    for k in (1..bridge_count).rev() {
        let j = rng.below(0, Purpose::BridgeOrder, k as u64, k as u64 + 1) as usize;
        order.swap(k, j);
    }
    let mut close = vec![0u64; bridge_count];
    for (position, &b) in order.iter().enumerate() {
        close[b] = day(r.bridges_from_day)
            + position as u64 * (day(r.bridges_to_day) - day(r.bridges_from_day))
                / bridge_count as u64;
    }
    let waves = u64::from(deepest) + 1;
    let rifts = (0..n)
        .filter(|&i| on_rift[i])
        .map(|i| {
            let bridge = bridge_of[i];
            let (shallows_epoch, deep_epoch) = if bridge > 0 {
                let epoch = close[usize::from(bridge) - 1];
                (epoch, epoch)
            } else {
                let d = u64::from(dist[i]);
                (
                    day(r.shallows_from_day)
                        + d * (day(r.deep_from_day) - day(r.shallows_from_day)) / waves,
                    day(r.deep_from_day)
                        + d * (day(r.bridges_from_day) - day(r.deep_from_day)) / waves,
                )
            };
            Rift {
                cell: i as u16,
                bridge,
                fault_epoch: day(r.fault_day),
                shallows_epoch,
                deep_epoch,
            }
        })
        .collect();
    let bridges = sites
        .iter()
        .enumerate()
        .map(|(b, &(center, a, c))| Bridge {
            id: b as u8 + 1,
            center: center as u16,
            plates: (a, c),
            close_epoch: close[b],
        })
        .collect();
    Plan {
        plates,
        plate_count: count,
        rifts,
        bridges,
    }
}
