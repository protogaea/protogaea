//! Miracles (spec §5, §13): what the naturalists' wishes do to the world, applied at the epoch
//! boundary after the natural events, in the order `weather` → `migrate` → `revive`.
//!
//! Each miracle is checked hard when it is applied ([`check`]); one that no longer holds is not
//! applied and the report says why. The same check serves the soft check of open wishes when a
//! window opens. Cooldowns (a weather region, a relocated clade, a revived entry) are part of
//! the state.

use serde::{Deserialize, Serialize};

use crate::genome::{GIFTS, TRAIT_COUNT, TRAIT_COUNT_V3, TRAIT_MAX};
use crate::rng::{derive, Purpose, Rng};
use crate::state::{Clade, CladeEffect, Cooldown, Effect, EffectKind, Organism, World};
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
    /// A patron's action for or against a clade around `center` (spec v0.3, draft): easing
    /// (`SHELTER`, `FORAGE`, `CURE`), harm (`BLIGHT`, `EXPOSE`, `SICKNESS`), or `GIFT + k` for the
    /// gift `genome::GIFTS[k]`.
    Clade {
        action: u8,
        clade_id: u32,
        center: u16,
    },
    /// Two clades crossed around `center` (spec v0.3 §5): hybrids founding a clade with two
    /// parents.
    Hybrid {
        clade_a: u32,
        clade_b: u32,
        center: u16,
    },
}

impl Miracle {
    /// The order of application: weather, then migrate, then revive.
    fn order(&self) -> u8 {
        match self {
            Miracle::Weather { .. } => 0,
            Miracle::Migrate { .. } => 1,
            Miracle::Revive { .. } => 2,
            Miracle::Clade { .. } => 3,
            Miracle::Hybrid { .. } => 4,
        }
    }
}

/// Cooldown kinds in the state.
pub const COOLDOWN_WEATHER: u8 = 0;
pub const COOLDOWN_MIGRATE: u8 = 1;
pub const COOLDOWN_MUSEUM: u8 = 2;
pub const COOLDOWN_SPORE: u8 = 3;
pub const COOLDOWN_RESPITE: u8 = 4;
pub const COOLDOWN_GIFT: u8 = 5;
pub const COOLDOWN_CURE: u8 = 6;
pub const COOLDOWN_HYBRID: u8 = 7;

/// Patrons' actions (spec v0.3, draft).
pub const SHELTER: u8 = 1;
pub const FORAGE: u8 = 2;
pub const CURE: u8 = 3;
pub const BLIGHT: u8 = 4;
pub const EXPOSE: u8 = 5;
pub const SICKNESS: u8 = 6;
pub const GIFT: u8 = 16;

/// The refusal of a bought `weather` under rules with patrons.
pub const WEATHER_IS_NATURAL: &str = "weather is natural in these rules";

/// Whether an action harms the clade it names.
pub fn is_harm(action: u8) -> bool {
    (BLIGHT..=SICKNESS).contains(&action)
}

/// The share multiplier of a patron's action (spec v0.3 §6) in percent, or `None` when the
/// action is not available against a clade of this share of the living (per mille) and size.
/// Help costs `max(75, s² / 100)` and is closed at 50% and above, except `cure`; harm costs ×4
/// against 3–5%, ×2 against 5–10%, ×1 against 10–20% and ×0.75 above, and small clades are
/// protected from it. A hybrid is priced as help for its first clade.
pub fn share_mult(action: u8, share_permille: u64, living: u32, rules: &Ruleset) -> Option<u64> {
    let p = rules.patrons.as_ref()?;
    if is_harm(action) {
        if living < p.protect_min_living || share_permille < u64::from(p.protect_min_permille) {
            return None;
        }
        return Some(match share_permille {
            0..50 => 400,
            50..100 => 200,
            100..200 => 100,
            _ => 75,
        });
    }
    if share_permille >= 500 && action != CURE {
        return None;
    }
    Some((share_permille * share_permille / 100).max(75))
}

/// The share multiplier of a miracle against the state it would apply to: [`share_mult`] for a
/// patron's action, 100 for the others.
pub fn price_share(world: &World, rules: &Ruleset, m: &Miracle) -> Option<u64> {
    let (action, clade) = match m {
        Miracle::Clade {
            action, clade_id, ..
        } => (*action, *clade_id),
        Miracle::Hybrid { clade_a, .. } => (SHELTER, *clade_a),
        _ => return Some(100),
    };
    let living = world.clades.get(&clade).map_or(0, |c| c.living);
    let share = u64::from(living) * 1000 / (world.organisms.len() as u64).max(1);
    share_mult(action, share, living, rules)
}

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

/// The cooldown key of a pair of clades, the same in either order.
pub fn pair_key(a: u32, b: u32) -> u32 {
    let (lo, hi) = (a.min(b), a.max(b));
    let d = derive(
        b"PROTOGAEA/HYBRID-PAIR/V0",
        &[&lo.to_le_bytes(), &hi.to_le_bytes()],
    );
    u32::from_le_bytes([d[0], d[1], d[2], d[3]])
}

/// Members of a clade within `radius` of `center`.
fn members_near(world: &World, clade_id: u32, center: usize, radius: i32) -> Vec<usize> {
    (0..world.organisms.len())
        .filter(|&k| {
            let o = &world.organisms[k];
            o.clade_id == clade_id && chebyshev(world, usize::from(o.cell), center) <= radius
        })
        .collect()
}

/// The gifts that at least half of these members hold.
fn common_gifts(world: &World, members: &[usize]) -> u8 {
    GIFTS
        .iter()
        .filter(|&&g| {
            2 * members
                .iter()
                .filter(|&&k| world.organisms[k].genome.has(g))
                .count()
                >= members.len()
        })
        .fold(0, |acc, &g| acc | g)
}

/// A hybrid of two reference genomes (spec v0.3 §5): each gene from one parent, by counter-based
/// randomness; then single steps (a point off a random trait above zero, or onto one below the
/// maximum) bring the traits to the budget; each gift in `gifts` passes with `gift_ppm`, up to
/// `max_gifts` in the order of `GIFTS`. The subject is the new clade.
#[allow(clippy::too_many_arguments)]
pub fn hybrid_genome(
    a: &Genome,
    b: &Genome,
    rules: &Ruleset,
    rng: &Rng,
    subject: u32,
    gifts: u8,
    gift_ppm: u32,
    max_gifts: u32,
) -> Genome {
    let subject = u64::from(subject);
    let from_b = |k: u32| rng.raw(0, Purpose::HybridGene, subject, k) & 1 == 1;
    let mut g = *a;
    for k in 0..TRAIT_COUNT {
        if from_b(k as u32) {
            g.traits[k] = b.traits[k];
        }
    }
    for k in 0..2 {
        if from_b((TRAIT_COUNT + k) as u32) {
            g.extra[k] = b.extra[k];
        }
    }
    let n = TRAIT_COUNT_V3 as u32;
    if from_b(n) {
        g.habitat = b.habitat;
    }
    if from_b(n + 1) {
        g.dispersal = b.dispersal;
    }
    if from_b(n + 2) {
        g.boldness = b.boldness;
    }
    if from_b(n + 3) {
        g.hue = b.hue;
    }
    // The traits in play: the six of v0.2, and size and longevity under v0.3 traits.
    let lanes = if rules.traits8.is_some() {
        TRAIT_COUNT_V3
    } else {
        TRAIT_COUNT
    };
    let get = |g: &Genome, k: usize| {
        if k < TRAIT_COUNT {
            g.traits[k]
        } else {
            g.extra[k - TRAIT_COUNT]
        }
    };
    let mut step = 0u64;
    loop {
        let sum: u32 = (0..lanes).map(|k| u32::from(get(&g, k))).sum();
        if sum == rules.trait_budget {
            break;
        }
        let over = sum > rules.trait_budget;
        let open: Vec<usize> = (0..lanes)
            .filter(|&k| {
                if over {
                    get(&g, k) > 0
                } else {
                    get(&g, k) < TRAIT_MAX
                }
            })
            .collect();
        let pick = open[rng.below(
            0,
            Purpose::HybridBudget,
            subject | step << 32,
            open.len() as u64,
        ) as usize];
        let v = if pick < TRAIT_COUNT {
            &mut g.traits[pick]
        } else {
            &mut g.extra[pick - TRAIT_COUNT]
        };
        if over {
            *v -= 1;
        } else {
            *v += 1;
        }
        step += 1;
    }
    g.gifts = 0;
    for (k, &gift) in GIFTS.iter().enumerate() {
        if gifts & gift != 0
            && g.gifts.count_ones() < max_gifts
            && rng.chance_ppm(0, Purpose::HybridGift, subject | (k as u64) << 32, gift_ppm)
        {
            g.gifts |= gift;
        }
    }
    g
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

/// The check of a miracle bought by a wish: [`check`], and with patrons weather is refused, as
/// it falls by itself (spec v0.3 §4.5).
pub fn check_wish(world: &World, rules: &Ruleset, m: &Miracle) -> Result<(), &'static str> {
    if rules.patrons.is_some() && matches!(m, Miracle::Weather { .. }) {
        return Err(WEATHER_IS_NATURAL);
    }
    check(world, rules, m)
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
        Miracle::Clade {
            action,
            clade_id,
            center,
        } => {
            let p = rules.patrons.as_ref().ok_or("no patrons in these rules")?;
            let center = usize::from(*center);
            if center >= cells {
                return Err("outside the map");
            }
            let clade = world
                .clades
                .get(clade_id)
                .filter(|c| c.living > 0)
                .ok_or("no such living clade")?;
            let r = i32::from(p.target_radius);
            let here = world
                .organisms
                .iter()
                .filter(|o| {
                    o.clade_id == *clade_id && chebyshev(world, usize::from(o.cell), center) <= r
                })
                .count();
            if here < p.min_members as usize {
                return Err("too few of the clade in the area");
            }
            match *action {
                SHELTER | FORAGE | CURE => {
                    if *action == CURE && cooling(world, COOLDOWN_CURE, |k| k == *clade_id) {
                        return Err("the clade was cured recently");
                    }
                    let reach = 2 * i32::from(p.area_radius);
                    if world.clade_effects.iter().any(|e| {
                        e.kind == *action
                            && e.clade_id == *clade_id
                            && chebyshev(world, usize::from(e.center), center) <= reach
                    }) {
                        return Err("the area overlaps an active effect");
                    }
                }
                BLIGHT | EXPOSE | SICKNESS => {
                    let population = world.organisms.len() as u64;
                    if clade.living < p.protect_min_living
                        || u64::from(clade.living) * 1000
                            < u64::from(p.protect_min_permille) * population
                    {
                        return Err("the clade is protected");
                    }
                    if cooling(world, COOLDOWN_RESPITE, |k| k == *clade_id) {
                        return Err("the clade rests from harm");
                    }
                }
                a if a >= GIFT && usize::from(a - GIFT) < GIFTS.len() => {
                    if cooling(world, COOLDOWN_GIFT, |k| k == *clade_id) {
                        return Err("the clade was gifted recently");
                    }
                }
                _ => return Err("no such action"),
            }
            Ok(())
        }
        Miracle::Hybrid {
            clade_a,
            clade_b,
            center,
        } => {
            let p = rules.patrons.as_ref().ok_or("no patrons in these rules")?;
            let center = usize::from(*center);
            if center >= cells {
                return Err("outside the map");
            }
            if clade_a == clade_b {
                return Err("a clade cannot cross with itself");
            }
            let living = |id: &u32| world.clades.get(id).filter(|c| c.living > 0);
            let (Some(a), Some(b)) = (living(clade_a), living(clade_b)) else {
                return Err("no such living clade");
            };
            let r = i32::from(p.target_radius);
            for id in [*clade_a, *clade_b] {
                if members_near(world, id, center, r).len() < p.hybrid_min_each as usize {
                    return Err("too few of a clade in the area");
                }
            }
            let d = a.reference.distance(&b.reference);
            if d < p.hybrid_min_distance {
                return Err("the clades are too close to cross");
            }
            if d > p.hybrid_max_distance {
                return Err("the clades are too far apart to cross");
            }
            if cooling(world, COOLDOWN_HYBRID, |k| {
                k == pair_key(*clade_a, *clade_b)
            }) {
                return Err("the pair was crossed recently");
            }
            let occ = occupancy(world);
            if !square(world, center, 1)
                .into_iter()
                .any(|c| world.cells[c].biome.is_land() && occ[c] < rules.max_per_cell)
            {
                return Err("no free land at the target");
            }
            Ok(())
        }
    }
}

/// Whether a refusal may pass by itself (an effect ends, a cooldown runs out, the area empties),
/// so that the wish can wait instead of being closed.
pub fn is_transient(reason: &str) -> bool {
    matches!(
        reason,
        "the area overlaps an active effect"
            | "the region is on cooldown"
            | "the clade was relocated recently"
            | "revived already this world day"
            | "too crowded around the start"
            | "no free land at the target"
            | "extinct too recently"
            | "the clade was cured recently"
            | "the clade rests from harm"
            | "the clade was gifted recently"
            | "the pair was crossed recently"
    )
}

/// What became of each miracle, in the order they were given.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcomes {
    pub applied: Vec<usize>,
    pub refused: Vec<(usize, &'static str)>,
}

/// Applies miracles at the epoch boundary, weather first, then migrations, revivals, patrons'
/// actions and hybrids; within a kind in the order given. Cooldowns that have run out are
/// dropped first. `rng` is the epoch's: hybrids draw from it.
pub fn apply(
    world: &mut World,
    rules: &Ruleset,
    rng: &Rng,
    miracles: &[Miracle],
    clades_founded: &mut Vec<u32>,
) -> Outcomes {
    let epoch = world.epoch;
    world.cooldowns.retain(|c| c.until > epoch);
    let mut order: Vec<usize> = (0..miracles.len()).collect();
    order.sort_by_key(|&i| miracles[i].order());
    let mut out = Outcomes::default();
    for i in order {
        match check_wish(world, rules, &miracles[i]) {
            Ok(()) => {
                apply_one(world, rules, rng, &miracles[i], clades_founded);
                out.applied.push(i);
            }
            Err(why) => out.refused.push((i, why)),
        }
    }
    out
}

pub(crate) fn apply_one(
    world: &mut World,
    rules: &Ruleset,
    rng: &Rng,
    m: &Miracle,
    clades_founded: &mut Vec<u32>,
) {
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
                        second_parent_id: 0,
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
        Miracle::Clade {
            action,
            clade_id,
            center,
        } => {
            let p = rules.patrons.as_ref().expect("checked");
            let tpe = u64::from(rules.ticks_per_epoch);
            if *action < GIFT {
                let ticks = if *action == CURE {
                    p.cure_ticks
                } else {
                    p.effect_ticks
                };
                world.clade_effects.push(CladeEffect {
                    kind: *action,
                    clade_id: *clade_id,
                    center: *center,
                    radius: p.area_radius,
                    remaining_ticks: ticks,
                });
                if is_harm(*action) {
                    world.cooldowns.push(Cooldown {
                        kind: COOLDOWN_RESPITE,
                        key: *clade_id,
                        until: epoch
                            + u64::from(p.effect_ticks).div_ceil(tpe)
                            + u64::from(p.harm_respite_epochs),
                    });
                } else if *action == CURE {
                    world.cooldowns.push(Cooldown {
                        kind: COOLDOWN_CURE,
                        key: *clade_id,
                        until: epoch + u64::from(p.cure_cooldown_epochs),
                    });
                }
            } else {
                // The gift goes to the youngest of the clade around the center that have room
                // (the highest ids): they have their lives ahead to pass it on.
                let gift = GIFTS[usize::from(action - GIFT)];
                let r = i32::from(p.target_radius);
                let center = usize::from(*center);
                let takers: Vec<usize> = (0..world.organisms.len())
                    .rev()
                    .filter(|&k| {
                        let o = &world.organisms[k];
                        o.clade_id == *clade_id
                            && !o.genome.has(gift)
                            && o.genome.gifts.count_ones() < p.max_gifts
                            && chebyshev(world, usize::from(o.cell), center) <= r
                    })
                    .take(p.gift_count as usize)
                    .collect();
                for k in takers {
                    world.organisms[k].genome.gifts |= gift;
                }
                world.cooldowns.push(Cooldown {
                    kind: COOLDOWN_GIFT,
                    key: *clade_id,
                    until: epoch + u64::from(p.gift_cooldown_epochs),
                });
            }
        }
        Miracle::Hybrid {
            clade_a,
            clade_b,
            center,
        } => {
            let p = rules.patrons.as_ref().expect("checked");
            let center = usize::from(*center);
            let r = i32::from(p.target_radius);
            let near_a = members_near(world, *clade_a, center, r);
            let near_b = members_near(world, *clade_b, center, r);
            let gifts = common_gifts(world, &near_a) | common_gifts(world, &near_b);
            let lineage = world.organisms[near_a[0]].lineage_id;
            let clade_id = world.next_clade_id;
            let genome = hybrid_genome(
                &world.clades[clade_a].reference,
                &world.clades[clade_b].reference,
                rules,
                rng,
                clade_id,
                gifts,
                p.hybrid_gift_ppm,
                p.max_gifts,
            );
            let mut occ = occupancy(world);
            let sites: Vec<usize> = square(world, center, 1)
                .into_iter()
                .filter(|&c| world.cells[c].biome.is_land())
                .collect();
            let mut placed = 0u32;
            'place: for _ in 0..p.hybrid_count {
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
                            lineage_id: lineage,
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
                        parent_id: *clade_a,
                        second_parent_id: *clade_b,
                        reference: genome,
                        founded_epoch: epoch,
                        living: placed,
                        peak_living: placed,
                    },
                );
                clades_founded.push(clade_id);
                if p.hybrid_vigor_ticks > 0 {
                    world.clade_effects.push(CladeEffect {
                        kind: FORAGE,
                        clade_id,
                        center: center as u16,
                        radius: p.area_radius,
                        remaining_ticks: p.hybrid_vigor_ticks,
                    });
                }
            }
            world.cooldowns.push(Cooldown {
                kind: COOLDOWN_HYBRID,
                key: pair_key(*clade_a, *clade_b),
                until: epoch + u64::from(p.hybrid_cooldown_epochs),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::Run;

    fn test_rng() -> Rng {
        Rng::new(&[3; 32])
    }

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
            &test_rng(),
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
        let out = apply(
            &mut w,
            &rules,
            &test_rng(),
            std::slice::from_ref(&m),
            &mut Vec::new(),
        );
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
        let out = apply(
            &mut w,
            &rules,
            &test_rng(),
            std::slice::from_ref(&m),
            &mut founded,
        );
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
            &test_rng(),
            &[revive.clone(), rain.clone()],
            &mut Vec::new(),
        );
        let ob = apply(
            &mut b,
            &rules,
            &test_rng(),
            &[rain, revive],
            &mut Vec::new(),
        );
        assert_eq!(a.state_root(), b.state_root());
        assert_eq!(oa.applied.len(), ob.applied.len());
    }
}

#[cfg(test)]
mod patron_tests {
    use super::*;
    use crate::genome::{CAMO, SWIM, VENOM};
    use crate::ruleset::Patrons;
    use crate::run::Run;

    fn test_rng() -> Rng {
        Rng::new(&[3; 32])
    }

    fn patron_rules() -> Ruleset {
        Ruleset {
            patrons: Some(Patrons::default()),
            ..Ruleset::default()
        }
    }

    /// The clade with the most members around one cell, and that cell.
    fn densest(w: &World) -> (u32, u16) {
        w.organisms
            .iter()
            .map(|o| {
                let n = w
                    .organisms
                    .iter()
                    .filter(|p| {
                        p.clade_id == o.clade_id
                            && chebyshev(w, usize::from(p.cell), usize::from(o.cell)) <= 2
                    })
                    .count();
                (n, o.clade_id, o.cell)
            })
            .max()
            .map(|(_, c, cell)| (c, cell))
            .expect("organisms")
    }

    #[test]
    fn easing_harm_and_gifts() {
        let rules = patron_rules();
        let mut run = Run::new(11, rules.clone());
        for _ in 0..3 {
            run.step();
        }
        let mut w = run.world;
        let (clade_id, center) = densest(&w);
        let act = |action| Miracle::Clade {
            action,
            clade_id,
            center,
        };

        // Without patrons in the rules the action does not exist.
        assert_eq!(
            check(&w, &Ruleset::default(), &act(SHELTER)),
            Err("no patrons in these rules")
        );

        let out = apply(
            &mut w,
            &rules,
            &test_rng(),
            &[act(SHELTER), act(SHELTER)],
            &mut Vec::new(),
        );
        assert_eq!(out.applied, vec![0]);
        assert_eq!(out.refused, vec![(1, "the area overlaps an active effect")]);
        assert_eq!(w.clade_effects.len(), 1);
        assert!(w.state_roots().clade_effects.is_some());

        let out = apply(
            &mut w,
            &rules,
            &test_rng(),
            &[act(GIFT), act(GIFT + 1)],
            &mut Vec::new(),
        );
        assert_eq!(out.applied, vec![0]);
        assert_eq!(out.refused, vec![(1, "the clade was gifted recently")]);
        let gifted = w.organisms.iter().filter(|o| o.genome.has(SWIM)).count();
        assert!((1..=10).contains(&gifted), "{gifted} gifted");

        let living = w.clades[&clade_id].living;
        let share = u64::from(living) * 1000 / w.organisms.len() as u64;
        if living >= 20 && share >= 20 {
            let out = apply(
                &mut w,
                &rules,
                &test_rng(),
                &[act(BLIGHT), act(EXPOSE)],
                &mut Vec::new(),
            );
            assert_eq!(out.applied, vec![0]);
            assert_eq!(out.refused, vec![(1, "the clade rests from harm")]);
        }
        let strict = Ruleset {
            patrons: Some(Patrons {
                protect_min_living: u32::MAX,
                ..Patrons::default()
            }),
            ..Ruleset::default()
        };
        w.cooldowns.clear();
        assert_eq!(
            check(&w, &strict, &act(SICKNESS)),
            Err("the clade is protected")
        );
    }

    #[test]
    fn gifts_are_inherited_and_weather_falls_by_itself() {
        let mut rules = patron_rules();
        rules.patrons.as_mut().expect("patrons").weather_ppm = 1_000_000;
        let mut run = Run::new(11, rules.clone());
        let first = run.step();
        assert!(first.natural_weather > 0, "every region gets weather");
        for _ in 0..2 {
            run.step();
        }
        let (clade_id, center) = densest(&run.world);
        let mut w = run.world.clone();
        let gift = Miracle::Clade {
            action: GIFT,
            clade_id,
            center,
        };
        let out = apply(&mut w, &rules, &test_rng(), &[gift], &mut Vec::new());
        assert_eq!(out.applied, vec![0]);
        assert!(w.organisms.iter().any(|o| o.genome.has(SWIM)));

        // Newborns inherit gifts and lose each with `gift_loss_ppm`.
        let parent = crate::Genome {
            gifts: SWIM,
            ..rules.founders[0].genome
        };
        let rng = crate::rng::Rng::new(&[7; 32]);
        let kept = (0..10_000u64)
            .filter(|&id| crate::genome::mutate(&parent, &rules, &rng, 0, id).has(SWIM))
            .count();
        assert!(
            (9_700..10_000).contains(&kept),
            "{kept} of 10,000 kept the gift"
        );
    }

    /// Clade `a` (the densest around `center`) and a new clade `b` three steps from it, with five
    /// members moved next to `a`'s.
    fn two_clades(w: &mut World, rules: &Ruleset) -> (u32, u32, u16) {
        let (a, center) = densest(w);
        let mut rb = w.clades[&a].reference;
        for _ in 0..3 {
            let from = (0..TRAIT_COUNT)
                .max_by_key(|&k| rb.traits[k])
                .expect("traits");
            let to = (0..TRAIT_COUNT)
                .min_by_key(|&k| rb.traits[k])
                .expect("traits");
            rb.traits[from] -= 1;
            rb.traits[to] += 1;
        }
        assert_eq!(w.clades[&a].reference.distance(&rb), 3);
        let b = w.next_clade_id;
        w.next_clade_id += 1;
        let mut occ = occupancy(w);
        let mut free: Vec<usize> = square(w, usize::from(center), 2)
            .into_iter()
            .filter(|&c| w.cells[c].biome.is_land())
            .collect();
        let movers: Vec<usize> = (0..w.organisms.len())
            .filter(|&k| w.organisms[k].clade_id != a)
            .take(5)
            .collect();
        for k in movers {
            let cell = free
                .iter()
                .copied()
                .find(|&c| occ[c] < rules.max_per_cell)
                .expect("room around the center");
            occ[cell] += 1;
            free.retain(|&c| occ[c] < rules.max_per_cell);
            let o = &mut w.organisms[k];
            if let Some(old) = w.clades.get_mut(&o.clade_id) {
                old.living -= 1;
            }
            o.cell = cell as u16;
            o.clade_id = b;
            o.genome = rb;
        }
        w.clades.insert(
            b,
            Clade {
                id: b,
                parent_id: 0,
                second_parent_id: 0,
                reference: rb,
                founded_epoch: w.epoch,
                living: 5,
                peak_living: 5,
            },
        );
        (a, b, center)
    }

    #[test]
    fn hybrids() {
        let rules = patron_rules();
        let mut run = Run::new(11, rules.clone());
        for _ in 0..3 {
            run.step();
        }
        let mut w = run.world;
        let (a, b, center) = two_clades(&mut w, &rules);
        let cross = |x, y| Miracle::Hybrid {
            clade_a: x,
            clade_b: y,
            center,
        };
        assert_eq!(
            check(&w, &Ruleset::default(), &cross(a, b)),
            Err("no patrons in these rules")
        );
        assert_eq!(
            check(&w, &rules, &cross(a, a)),
            Err("a clade cannot cross with itself")
        );
        assert_eq!(pair_key(a, b), pair_key(b, a));

        let organisms = w.organisms.len();
        let mut founded = Vec::new();
        let out = apply(
            &mut w,
            &rules,
            &test_rng(),
            &[cross(a, b), cross(b, a)],
            &mut founded,
        );
        // The same pair in either order is crossed once.
        assert_eq!(out.applied, vec![0]);
        assert_eq!(out.refused, vec![(1, "the pair was crossed recently")]);
        let h = w.clades[&founded[0]];
        assert_eq!((h.parent_id, h.second_parent_id), (a, b));
        let p = rules.patrons.as_ref().expect("patrons");
        assert_eq!(h.living, p.hybrid_count);
        assert_eq!(w.organisms.len(), organisms + p.hybrid_count as usize);
        assert!(h.reference.is_valid(rules.trait_budget));
        // Hybrid vigor: forage for the new clade around the crossing.
        assert!(w.clade_effects.iter().any(|e| e.kind == FORAGE
            && e.clade_id == h.id
            && e.remaining_ticks == p.hybrid_vigor_ticks));
        // A hybrid clade has its second parent in the state; others hash as before.
        assert_eq!(clade_bytes_len(&h), clade_bytes_len(&w.clades[&a]) + 4);

        // Too close to cross: the same reference genome.
        w.clades.get_mut(&b).expect("b").reference = w.clades[&a].reference;
        w.cooldowns.clear();
        assert_eq!(
            check(&w, &rules, &cross(a, b)),
            Err("the clades are too close to cross")
        );
    }

    fn clade_bytes_len(c: &Clade) -> usize {
        crate::state::clade_bytes_for_test(c).len()
    }

    #[test]
    fn hybrid_genomes_keep_the_budget_and_pass_gifts_by_chance() {
        let rules = Ruleset::v03();
        let rng = test_rng();
        let genome = |traits, extra| Genome {
            traits,
            habitat: 0,
            dispersal: 1,
            boldness: 2,
            hue: 90,
            gifts: 0,
            extra,
        };
        let a = genome([8, 8, 0, 0, 8, 0], [8, 0]);
        let b = genome([0, 0, 8, 8, 0, 8], [0, 8]);
        let mut swim = 0;
        for subject in 0..600 {
            let g = hybrid_genome(
                &a,
                &b,
                &rules,
                &rng,
                subject,
                SWIM | VENOM | CAMO,
                500_000,
                2,
            );
            assert!(g.is_valid(rules.trait_budget), "{g:?}");
            assert!(g.gifts.count_ones() <= 2);
            assert_eq!(
                g,
                hybrid_genome(
                    &a,
                    &b,
                    &rules,
                    &rng,
                    subject,
                    SWIM | VENOM | CAMO,
                    500_000,
                    2
                )
            );
            swim += u32::from(g.has(SWIM));
        }
        // The first gift in order is never held back by the limit: it passes half the time.
        assert!(
            (240..=360).contains(&swim),
            "swim passed {swim} times in 600"
        );
    }

    #[test]
    fn the_share_rule_prices_patrons_wishes() {
        let rules = patron_rules();
        let mut run = Run::new(11, rules.clone());
        run.step();
        let w = &run.world;
        let (big, center) = densest(w);
        let living = w.clades[&big].living;
        let share = u64::from(living) * 1000 / w.organisms.len() as u64;
        let help = |action, clade_id| Miracle::Clade {
            action,
            clade_id,
            center,
        };
        assert_eq!(
            price_share(w, &rules, &help(SHELTER, big)),
            Some((share * share / 100).max(75))
        );
        // A hybrid is priced as help for its first clade; other miracles have no share.
        let cross = Miracle::Hybrid {
            clade_a: big,
            clade_b: big + 1,
            center,
        };
        assert_eq!(
            price_share(w, &rules, &cross),
            price_share(w, &rules, &help(SHELTER, big))
        );
        let rain = Miracle::Weather { center, rain: true };
        assert_eq!(price_share(w, &rules, &rain), Some(100));
        // No harm against a clade below protection, and none at all without patrons.
        assert_eq!(share_mult(BLIGHT, 29, 1000, &rules), None);
        assert_eq!(share_mult(BLIGHT, 30, 19, &rules), None);
        assert_eq!(share_mult(BLIGHT, 30, 20, &rules), Some(400));
        assert_eq!(share_mult(BLIGHT, 250, 500, &rules), Some(75));
        assert_eq!(share_mult(SHELTER, 500, 500, &rules), None);
        assert_eq!(share_mult(CURE, 500, 500, &rules), Some(2500));
        assert_eq!(share_mult(SHELTER, 200, 100, &Ruleset::default()), None);
        // Bought weather is refused when weather falls by itself.
        assert_eq!(check_wish(w, &rules, &rain), Err(WEATHER_IS_NATURAL));
        assert!(!is_transient(WEATHER_IS_NATURAL));
        assert_eq!(
            check_wish(w, &Ruleset::default(), &rain),
            check(w, &Ruleset::default(), &rain)
        );
    }
}
