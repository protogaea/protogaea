//! Miracles (spec §5, §13): what the naturalists' wishes do to the world, applied at the epoch
//! boundary after the natural events, in the order `weather` → `migrate` → `revive`.
//!
//! Each miracle is checked hard when it is applied ([`check`]); one that no longer holds is not
//! applied and the report says why. The same check serves the soft check of open wishes when a
//! window opens. Cooldowns (a weather region, a relocated clade, a revived entry) are part of
//! the state.

use serde::{Deserialize, Serialize};

use crate::genome::{TRAIT_COUNT, TRAIT_MAX};
use crate::state::{Clade, Cooldown, Effect, EffectKind, Organism, World};
use crate::{Genome, Ruleset};

/// A miracle as the core applies it. Cells are indexes `x + y · width`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Miracle {
    /// Rain or drought over the square of `weather_radius` around `center`.
    Weather { center: u16, rain: bool },
    /// Up to `migrate_moved` organisms of a clade from around `from` to around `to`.
    Migrate { clade_id: u32, from: u16, to: u16 },
    /// A museum entry (a clade id) or a spore bank entry (an index) comes back near `at`, its
    /// genome edited by at most two steps, each moving one point from trait `i` to trait `j`.
    Revive {
        from_museum: bool,
        entry_id: u32,
        steps: Vec<(u8, u8)>,
        at: u16,
    },
}

impl Miracle {
    /// The order of application: weather, then migrate, then revive.
    fn order(&self) -> u8 {
        match self {
            Miracle::Weather { .. } => 0,
            Miracle::Migrate { .. } => 1,
            Miracle::Revive { .. } => 2,
        }
    }
}

/// Cooldown kinds in the state.
pub const COOLDOWN_WEATHER: u8 = 0;
pub const COOLDOWN_MIGRATE: u8 = 1;
pub const COOLDOWN_MUSEUM: u8 = 2;
pub const COOLDOWN_SPORE: u8 = 3;

fn chebyshev(world: &World, a: usize, b: usize) -> i32 {
    let ((ax, ay), (bx, by)) = (world.coords(a), world.coords(b));
    (ax - bx).abs().max((ay - by).abs())
}

/// Cells within `radius` (a square) of `center`, row by row.
fn square(world: &World, center: usize, radius: i32) -> Vec<usize> {
    let (cx, cy) = world.coords(center);
    let mut out = Vec::new();
    for y in cy - radius..=cy + radius {
        for x in cx - radius..=cx + radius {
            if let Some(c) = world.index(x, y) {
                out.push(c);
            }
        }
    }
    out
}

fn occupancy(world: &World) -> Vec<u8> {
    let mut occ = vec![0u8; world.cells.len()];
    for o in &world.organisms {
        occ[usize::from(o.cell)] = occ[usize::from(o.cell)].saturating_add(1);
    }
    occ
}

fn cooling(world: &World, kind: u8, key: impl Fn(u32) -> bool) -> bool {
    world
        .cooldowns
        .iter()
        .any(|c| c.kind == kind && c.until > world.epoch && key(c.key))
}

/// The genome a revival starts from, after its edit steps; `None` if a step is not possible.
fn edited(base: &Genome, steps: &[(u8, u8)]) -> Option<Genome> {
    if steps.len() > 2 {
        return None;
    }
    let mut g = *base;
    for &(i, j) in steps {
        let (i, j) = (usize::from(i), usize::from(j));
        if i >= TRAIT_COUNT
            || j >= TRAIT_COUNT
            || i == j
            || g.traits[i] == 0
            || g.traits[j] >= TRAIT_MAX
        {
            return None;
        }
        g.traits[i] -= 1;
        g.traits[j] += 1;
    }
    Some(g)
}

/// The base genome of a revival and the clade it descends from (0 for the spore bank).
fn revival_base(
    world: &World,
    rules: &Ruleset,
    from_museum: bool,
    entry_id: u32,
) -> Result<(Genome, u32), &'static str> {
    if from_museum {
        let entry = world
            .museum
            .iter()
            .find(|m| m.clade_id == entry_id)
            .ok_or("no such museum entry")?;
        if world.epoch < entry.extinct_epoch + u64::from(rules.miracles.revive_extinct_epochs) {
            return Err("extinct too recently");
        }
        Ok((entry.reference, entry.clade_id))
    } else {
        let spore = world
            .spore_bank
            .get(entry_id as usize)
            .ok_or("no such spore bank entry")?;
        Ok((spore.genome, 0))
    }
}

/// The hard check of a miracle against the state it would apply to (spec §5).
pub fn check(world: &World, rules: &Ruleset, m: &Miracle) -> Result<(), &'static str> {
    let mr = &rules.miracles;
    let cells = world.cells.len();
    match m {
        Miracle::Weather { center, .. } => {
            let center = usize::from(*center);
            if center >= cells {
                return Err("outside the map");
            }
            let r = i32::from(mr.weather_radius);
            if world
                .effects
                .iter()
                .any(|e| chebyshev(world, usize::from(e.center), center) <= i32::from(e.radius) + r)
            {
                return Err("the area overlaps an active effect");
            }
            if cooling(world, COOLDOWN_WEATHER, |k| {
                chebyshev(world, k as usize, center) <= 2 * r
            }) {
                return Err("the region is on cooldown");
            }
            Ok(())
        }
        Miracle::Migrate { clade_id, from, to } => {
            let (from, to) = (usize::from(*from), usize::from(*to));
            if from >= cells || to >= cells {
                return Err("outside the map");
            }
            if !world.clades.get(clade_id).is_some_and(|c| c.living > 0) {
                return Err("no such living clade");
            }
            let r = i32::from(mr.migrate_radius);
            let here = world
                .organisms
                .iter()
                .filter(|o| {
                    o.clade_id == *clade_id && chebyshev(world, usize::from(o.cell), from) <= r
                })
                .count();
            if here < mr.migrate_min as usize {
                return Err("too few of the clade in the source area");
            }
            if cooling(world, COOLDOWN_MIGRATE, |k| k == *clade_id) {
                return Err("the clade was relocated recently");
            }
            let occ = occupancy(world);
            if !square(world, to, 1)
                .into_iter()
                .any(|c| world.cells[c].biome.is_land() && occ[c] < rules.max_per_cell)
            {
                return Err("no free land at the target");
            }
            Ok(())
        }
        Miracle::Revive {
            from_museum,
            entry_id,
            steps,
            at,
        } => {
            let at = usize::from(*at);
            if at >= cells || !world.cells[at].biome.is_land() {
                return Err("the start is not land");
            }
            let (base, _) = revival_base(world, rules, *from_museum, *entry_id)?;
            if edited(&base, steps).is_none() {
                return Err("an edit step is not possible");
            }
            let kind = if *from_museum {
                COOLDOWN_MUSEUM
            } else {
                COOLDOWN_SPORE
            };
            if cooling(world, kind, |k| k == *entry_id) {
                return Err("revived already this world day");
            }
            let r = i32::from(mr.revive_radius);
            let nearby = world
                .organisms
                .iter()
                .filter(|o| chebyshev(world, usize::from(o.cell), at) <= r)
                .count();
            if nearby > mr.revive_max_nearby as usize {
                return Err("too crowded around the start");
            }
            Ok(())
        }
    }
}

/// What became of each miracle, in the order they were given.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcomes {
    pub applied: Vec<usize>,
    pub refused: Vec<(usize, &'static str)>,
}

/// Applies miracles at the epoch boundary, weather first, then migrations, then revivals; within
/// a kind in the order given. Cooldowns that have run out are dropped first.
pub fn apply(
    world: &mut World,
    rules: &Ruleset,
    miracles: &[Miracle],
    clades_founded: &mut Vec<u32>,
) -> Outcomes {
    let epoch = world.epoch;
    world.cooldowns.retain(|c| c.until > epoch);
    let mut order: Vec<usize> = (0..miracles.len()).collect();
    order.sort_by_key(|&i| miracles[i].order());
    let mut out = Outcomes::default();
    for i in order {
        match check(world, rules, &miracles[i]) {
            Ok(()) => {
                apply_one(world, rules, &miracles[i], clades_founded);
                out.applied.push(i);
            }
            Err(why) => out.refused.push((i, why)),
        }
    }
    out
}

fn apply_one(world: &mut World, rules: &Ruleset, m: &Miracle, clades_founded: &mut Vec<u32>) {
    let mr = &rules.miracles;
    let epoch = world.epoch;
    match m {
        Miracle::Weather { center, rain } => {
            let r = i32::from(mr.weather_radius);
            for c in square(world, usize::from(*center), r) {
                let m = &mut world.cells[c].moisture;
                *m = if *rain {
                    m.saturating_add(mr.weather_moisture).min(100)
                } else {
                    m.saturating_sub(mr.weather_moisture)
                };
            }
            world.effects.push(Effect {
                kind: if *rain {
                    EffectKind::Rain
                } else {
                    EffectKind::Dry
                },
                center: *center,
                radius: mr.weather_radius,
                remaining_ticks: mr.weather_ticks,
            });
            let lasts = u64::from(mr.weather_ticks).div_ceil(u64::from(rules.ticks_per_epoch));
            world.cooldowns.push(Cooldown {
                kind: COOLDOWN_WEATHER,
                key: u32::from(*center),
                until: epoch + lasts + u64::from(mr.cooldown_epochs),
            });
        }
        Miracle::Migrate { clade_id, from, to } => {
            let (from, to) = (usize::from(*from), usize::from(*to));
            let r = i32::from(mr.migrate_radius);
            let mut occ = occupancy(world);
            let targets: Vec<usize> = square(world, to, 1)
                .into_iter()
                .filter(|&c| world.cells[c].biome.is_land())
                .collect();
            // The lowest ids in the source area move, each to the first target with room.
            let movers: Vec<usize> = (0..world.organisms.len())
                .filter(|&k| {
                    let o = &world.organisms[k];
                    o.clade_id == *clade_id && chebyshev(world, usize::from(o.cell), from) <= r
                })
                .take(mr.migrate_moved as usize)
                .collect();
            for k in movers {
                let Some(&cell) = targets.iter().find(|&&c| occ[c] < rules.max_per_cell) else {
                    break;
                };
                let o = &mut world.organisms[k];
                occ[usize::from(o.cell)] -= 1;
                occ[cell] += 1;
                o.cell = cell as u16;
            }
            world.cooldowns.push(Cooldown {
                kind: COOLDOWN_MIGRATE,
                key: *clade_id,
                until: epoch + u64::from(mr.cooldown_epochs),
            });
        }
        Miracle::Revive {
            from_museum,
            entry_id,
            steps,
            at,
        } => {
            let (base, parent) =
                revival_base(world, rules, *from_museum, *entry_id).expect("checked");
            let genome = edited(&base, steps).expect("checked");
            let mut occ = occupancy(world);
            let sites: Vec<usize> = square(world, usize::from(*at), 1)
                .into_iter()
                .filter(|&c| world.cells[c].biome.is_land())
                .collect();
            let clade_id = world.next_clade_id;
            let mut placed = 0u32;
            'place: for _ in 0..mr.revive_count {
                if world.organisms.len() >= rules.max_organisms as usize {
                    break;
                }
                for &c in &sites {
                    if occ[c] < rules.max_per_cell {
                        occ[c] += 1;
                        let id = world.next_organism_id;
                        world.next_organism_id = id.checked_add(1).expect("organism id overflow");
                        world.organisms.push(Organism {
                            id,
                            parent_id: 0,
                            lineage_id: 0,
                            clade_id,
                            cell: c as u16,
                            age: 0,
                            energy: rules.genesis_energy,
                            genome,
                        });
                        placed += 1;
                        continue 'place;
                    }
                }
                break;
            }
            if placed > 0 {
                world.next_clade_id = clade_id.checked_add(1).expect("clade id overflow");
                world.clades.insert(
                    clade_id,
                    Clade {
                        id: clade_id,
                        parent_id: parent,
                        reference: genome,
                        founded_epoch: epoch,
                        living: placed,
                        peak_living: placed,
                    },
                );
                clades_founded.push(clade_id);
            }
            world.cooldowns.push(Cooldown {
                kind: if *from_museum {
                    COOLDOWN_MUSEUM
                } else {
                    COOLDOWN_SPORE
                },
                key: *entry_id,
                until: epoch + u64::from(rules.epochs_per_day),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::Run;

    fn world() -> (World, Ruleset) {
        let rules = Ruleset::default();
        let mut run = Run::new(11, rules.clone());
        for _ in 0..3 {
            run.step();
        }
        (run.world, rules)
    }

    fn land(world: &World) -> u16 {
        // A land cell with land all around, far from active effects.
        (0..world.cells.len())
            .find(|&c| {
                square(world, c, 3).len() == 49
                    && square(world, c, 3)
                        .iter()
                        .all(|&n| world.cells[n].biome.is_land())
                    && !world.effects.iter().any(|e| {
                        chebyshev(world, usize::from(e.center), c) <= i32::from(e.radius) + 3
                    })
            })
            .expect("some land") as u16
    }

    #[test]
    fn weather_applies_once_per_region_and_cools_down() {
        let (mut w, rules) = world();
        let c = land(&w);
        let before = w.cells[usize::from(c)].moisture;
        let rain = Miracle::Weather {
            center: c,
            rain: true,
        };
        let out = apply(
            &mut w,
            &rules,
            &[rain.clone(), rain.clone()],
            &mut Vec::new(),
        );
        assert_eq!(out.applied, vec![0]);
        assert_eq!(out.refused, vec![(1, "the area overlaps an active effect")]);
        assert_eq!(
            w.cells[usize::from(c)].moisture,
            before.saturating_add(30).min(100)
        );
        assert!(w.effects.iter().any(|e| e.kind == EffectKind::Rain));
        // After the effect ends the region is still on cooldown.
        w.effects.retain(|e| e.kind != EffectKind::Rain);
        assert_eq!(check(&w, &rules, &rain), Err("the region is on cooldown"));
        w.epoch += 3 + 12;
        w.cooldowns.retain(|cd| cd.until > w.epoch);
        assert_eq!(check(&w, &rules, &rain), Ok(()));
    }

    #[test]
    fn migrate_moves_three_and_needs_ten() {
        let (mut w, rules) = world();
        // The clade with the most members in some 5 × 5 area.
        let (clade_id, from) = w
            .organisms
            .iter()
            .map(|o| {
                let n = w
                    .organisms
                    .iter()
                    .filter(|p| {
                        p.clade_id == o.clade_id
                            && chebyshev(&w, usize::from(p.cell), usize::from(o.cell)) <= 2
                    })
                    .count();
                (n, o.clade_id, o.cell)
            })
            .max()
            .map(|(_, c, cell)| (c, cell))
            .unwrap();
        let to = land(&w);
        let m = Miracle::Migrate { clade_id, from, to };
        assert_eq!(
            check(&w, &rules, &m),
            Ok(()),
            "a dense clade exists after three epochs"
        );
        let count_near = |w: &World, c: u16| {
            w.organisms
                .iter()
                .filter(|o| {
                    o.clade_id == clade_id && chebyshev(w, usize::from(o.cell), usize::from(c)) <= 1
                })
                .count()
        };
        let before_total = w.organisms.len();
        let near_before = count_near(&w, to);
        let out = apply(&mut w, &rules, std::slice::from_ref(&m), &mut Vec::new());
        assert_eq!(out.applied, vec![0]);
        assert_eq!(w.organisms.len(), before_total, "a move, not a copy");
        assert_eq!(count_near(&w, to), near_before + 3);
        assert_eq!(
            check(&w, &rules, &m),
            Err("the clade was relocated recently")
        );
        let lonely = Miracle::Migrate {
            clade_id: u32::MAX,
            from,
            to,
        };
        assert_eq!(check(&w, &rules, &lonely), Err("no such living clade"));
    }

    #[test]
    fn revive_from_the_spore_bank_founds_a_clade() {
        let (mut w, rules) = world();
        let at = land(&w);
        // Clear the area, so the crowding rule does not refuse it.
        let (width, (ax, ay)) = (i32::from(w.width), w.coords(usize::from(at)));
        w.organisms.retain(|o| {
            let c = i32::from(o.cell);
            (c % width - ax).abs().max((c / width - ay).abs()) > 2
        });
        let base = w.spore_bank[0].genome;
        let (i, j) = (0..6u8)
            .flat_map(|i| (0..6u8).map(move |j| (i, j)))
            .find(|&(i, j)| {
                i != j && base.traits[usize::from(i)] > 0 && base.traits[usize::from(j)] < TRAIT_MAX
            })
            .unwrap();
        let m = Miracle::Revive {
            from_museum: false,
            entry_id: 0,
            steps: vec![(i, j)],
            at,
        };
        let next = w.next_clade_id;
        let mut founded = Vec::new();
        let out = apply(&mut w, &rules, std::slice::from_ref(&m), &mut founded);
        assert_eq!(out.applied, vec![0]);
        assert_eq!(founded, vec![next]);
        let clade = &w.clades[&next];
        assert_eq!(clade.living, 5);
        assert_eq!(
            clade.reference.traits[usize::from(i)],
            base.traits[usize::from(i)] - 1
        );
        assert!(
            clade.reference.is_valid(rules.trait_budget),
            "an edit keeps the trait budget"
        );
        assert_eq!(check(&w, &rules, &m), Err("revived already this world day"));
        let bad = Miracle::Revive {
            from_museum: false,
            entry_id: 0,
            steps: vec![(1, 1)],
            at,
        };
        assert_eq!(check(&w, &rules, &bad), Err("an edit step is not possible"));
    }

    /// The order of application does not depend on the order given: a revival listed before a
    /// weather miracle still comes after it.
    #[test]
    fn order_is_weather_migrate_revive() {
        let (w, rules) = world();
        let at = land(&w);
        let revive = Miracle::Revive {
            from_museum: false,
            entry_id: 1,
            steps: vec![],
            at,
        };
        let rain = Miracle::Weather {
            center: at,
            rain: true,
        };
        let (mut a, mut b) = (w.clone(), w);
        let oa = apply(
            &mut a,
            &rules,
            &[revive.clone(), rain.clone()],
            &mut Vec::new(),
        );
        let ob = apply(&mut b, &rules, &[rain, revive], &mut Vec::new());
        assert_eq!(a.state_root(), b.state_root());
        assert_eq!(oa.applied.len(), ob.applied.len());
    }
}
