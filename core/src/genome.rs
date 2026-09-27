//! The genome (spec §11.1).

use serde::{Deserialize, Serialize};

use crate::rng::{Purpose, Rng};
use crate::ruleset::Ruleset;

pub const TRAIT_COUNT: usize = 6;
pub const MOVEMENT: usize = 0;
pub const PERCEPTION: usize = 1;
pub const PLANT: usize = 2;
pub const HUNTING: usize = 3;
pub const DEFENSE: usize = 4;
pub const FERTILITY: usize = 5;
pub const TRAIT_MAX: u8 = 8;
/// The largest trait budget: every trait at its maximum.
pub const TRAIT_BUDGET_MAX: u32 = TRAIT_COUNT as u32 * TRAIT_MAX as u32;
pub const HABITAT_GENERALIST: u8 = 5;
pub const DISPERSAL_MAX: u8 = 3;
pub const BOLDNESS_MAX: u8 = 3;
pub const HUE_RANGE: u16 = 360;

/// Gifts (spec v0.3, draft): heritable abilities outside the trait budget, one bit each.
pub const SWIM: u8 = 1;
pub const VENOM: u8 = 2;
pub const CAMO: u8 = 4;
pub const KEEN: u8 = 8;
pub const HARDY: u8 = 16;
pub const SCAVENGE: u8 = 32;
/// The gifts in the order of `Patrons::gift_upkeep`.
pub const GIFTS: [u8; 6] = [SWIM, VENOM, CAMO, KEEN, HARDY, SCAVENGE];

fn no_gifts(g: &u8) -> bool {
    *g == 0
}

/// The v0.3 traits outside `traits` (spec v0.3, draft): size and longevity.
pub const SIZE: usize = 0;
pub const LONGEVITY: usize = 1;
/// All traits under v0.3 rules: the six of v0.2 and the two of `extra`.
pub const TRAIT_COUNT_V3: usize = TRAIT_COUNT + 2;

fn no_extra(e: &[u8; 2]) -> bool {
    *e == [0, 0]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Genome {
    /// Movement, perception, plant eating, hunting, defense and fertility: each 0–8, summing
    /// to the trait budget of the rules (`Ruleset::trait_budget`).
    pub traits: [u8; TRAIT_COUNT],
    /// The preferred land biome, 0–4, or 5 for a generalist.
    pub habitat: u8,
    /// 0–3.
    pub dispersal: u8,
    /// 0–3.
    pub boldness: u8,
    /// A neutral color, 0–359.
    pub hue: u16,
    /// Gifts held (spec v0.3, draft), as bits of `GIFTS`. Absent in worlds without patrons, so
    /// their genomes, roots and rulesets read as before.
    #[serde(default, skip_serializing_if = "no_gifts")]
    pub gifts: u8,
    /// Size and longevity (spec v0.3, draft), within the trait budget under v0.3 rules; zero and
    /// absent otherwise, so v0.2 genomes read and hash as before.
    #[serde(default, skip_serializing_if = "no_extra")]
    pub extra: [u8; 2],
}

impl Genome {
    /// Every gene in range, and the traits spend exactly `trait_budget`.
    pub fn is_valid(&self, trait_budget: u32) -> bool {
        self.traits
            .iter()
            .chain(&self.extra)
            .all(|&t| t <= TRAIT_MAX)
            && self
                .traits
                .iter()
                .chain(&self.extra)
                .map(|&t| u32::from(t))
                .sum::<u32>()
                == trait_budget
            && self.habitat <= HABITAT_GENERALIST
            && self.dispersal <= DISPERSAL_MAX
            && self.boldness <= BOLDNESS_MAX
            && self.hue < HUE_RANGE
            && self.gifts < 64
    }

    pub fn has(&self, gift: u8) -> bool {
        self.gifts & gift != 0
    }

    /// The number of mutation steps between two genomes: half the sum of the absolute trait
    /// differences. Behavioral genes and hue do not count.
    pub fn distance(&self, other: &Genome) -> u32 {
        let total: u32 = self
            .traits
            .iter()
            .chain(&self.extra)
            .zip(other.traits.iter().chain(&other.extra))
            .map(|(&a, &b)| u32::from(a.abs_diff(b)))
            .sum();
        total / 2
    }

    /// Steps per tick: `(M + 2) / 3`.
    pub fn steps(&self) -> u32 {
        u32::from(self.traits[MOVEMENT]).div_ceil(3)
    }

    /// Sight radius: `1 + P / 3`.
    pub fn sight(&self) -> i32 {
        1 + i32::from(self.traits[PERCEPTION]) / 3 + i32::from(self.has(KEEN))
    }

    /// `H × attack_weight + P`, before the roll.
    pub fn attack(&self, rules: &Ruleset) -> i32 {
        let size = rules
            .traits8
            .as_ref()
            .map_or(0, |t| i32::from(self.extra[SIZE]) * t.size_attack);
        i32::from(self.traits[HUNTING]) * rules.attack_weight
            + i32::from(self.traits[PERCEPTION])
            + size
    }

    /// The most energy the organism can hold: `energy_max`, scaled by size under v0.3 rules.
    pub fn energy_cap(&self, rules: &Ruleset) -> i32 {
        match &rules.traits8 {
            Some(t) => {
                rules.energy_max
                    * (t.size_energy_base_pct + t.size_energy_pct * i32::from(self.extra[SIZE]))
                    / 100
            }
            None => rules.energy_max,
        }
    }

    /// When aging starts and the age of certain death, scaled by longevity under v0.3 rules.
    pub fn lifespan(&self, rules: &Ruleset) -> (u32, u32) {
        match &rules.traits8 {
            Some(t) => {
                let pct = t.longevity_base_pct + t.longevity_pct * u32::from(self.extra[LONGEVITY]);
                (
                    rules.senescence_start * pct / 100,
                    rules.max_age * pct / 100,
                )
            }
            None => (rules.senescence_start, rules.max_age),
        }
    }

    /// The energy needed to breed, and the newborn's energy: bigger organisms breed later and
    /// give more.
    pub fn breeding(&self, rules: &Ruleset) -> (i32, i32) {
        let f = i32::from(self.traits[FERTILITY]);
        let s = rules.traits8.as_ref().map_or((0, 0), |t| {
            (
                i32::from(self.extra[SIZE]) * t.size_repro,
                i32::from(self.extra[SIZE]) * t.size_child,
            )
        });
        (
            rules.repro_base - f * rules.repro_per_fertility + s.0,
            rules.child_base - f * rules.child_per_fertility + s.1,
        )
    }

    /// `D × defense_weight + M + P / 2`, before the roll.
    pub fn defense(&self, rules: &Ruleset) -> i32 {
        let mut d = i32::from(self.traits[DEFENSE]) * rules.defense_weight
            + i32::from(self.traits[MOVEMENT])
            + i32::from(self.traits[PERCEPTION]) / 2;
        if let Some(t) = &rules.traits8 {
            d += i32::from(self.extra[SIZE]) * t.size_defense;
        }
        if self.gifts != 0 {
            if let Some(p) = &rules.patrons {
                d += i32::from(self.has(VENOM)) * p.venom_defense
                    + i32::from(self.has(CAMO)) * p.camo_defense
                    + i32::from(self.has(KEEN)) * p.keen_defense;
            }
        }
        d
    }

    /// Base metabolism plus trait upkeep, per tick, before habitat and movement.
    pub fn upkeep(&self, rules: &Ruleset) -> i32 {
        let mut total = rules.base_metabolism;
        for (k, &t) in self.traits.iter().enumerate() {
            total += rules.trait_upkeep[k] * i32::from(t);
        }
        if let Some(t) = &rules.traits8 {
            total += t.size_upkeep * i32::from(self.extra[SIZE])
                + t.longevity_upkeep * i32::from(self.extra[LONGEVITY]);
        }
        if self.gifts != 0 {
            if let Some(p) = &rules.patrons {
                for (k, &gift) in GIFTS.iter().enumerate() {
                    if self.has(gift) {
                        total += p.gift_upkeep[k];
                    }
                }
            }
        }
        total
    }
}

/// Copies a parent's genome with mutations (spec §11.1). The subject is the child's id.
pub fn mutate(parent: &Genome, rules: &Ruleset, rng: &Rng, tick: u32, subject: u64) -> Genome {
    let mut g = *parent;
    if rng.chance_ppm(tick, Purpose::MutationChance, subject, rules.mutation_ppm) {
        // One step between a valid pair: choose uniformly among pairs that stay in range. Under
        // v0.3 rules size and longevity take part.
        let count = if rules.traits8.is_some() {
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
        let mut pairs = [(0usize, 0usize); TRAIT_COUNT_V3 * (TRAIT_COUNT_V3 - 1)];
        let mut n = 0;
        for i in 0..count {
            for j in 0..count {
                if i != j && get(&g, i) < TRAIT_MAX && get(&g, j) > 0 {
                    pairs[n] = (i, j);
                    n += 1;
                }
            }
        }
        if n > 0 {
            let (i, j) = pairs[rng.below(tick, Purpose::MutationPair, subject, n as u64) as usize];
            for (k, up) in [(i, true), (j, false)] {
                let v = if k < TRAIT_COUNT {
                    &mut g.traits[k]
                } else {
                    &mut g.extra[k - TRAIT_COUNT]
                };
                if up {
                    *v += 1;
                } else {
                    *v -= 1;
                }
            }
        }
    }
    if rng.chance_ppm(
        tick,
        Purpose::BehaviorChance,
        subject,
        rules.behavior_mutation_ppm,
    ) {
        match rng.below(tick, Purpose::BehaviorGene, subject, 3) {
            0 => {
                // A different value among the six: one of the five others.
                let v = rng.below(
                    tick,
                    Purpose::BehaviorValue,
                    subject,
                    u64::from(HABITAT_GENERALIST),
                ) as u8;
                g.habitat = if v >= g.habitat { v + 1 } else { v };
            }
            1 => {
                let up = rng.below(tick, Purpose::BehaviorValue, subject, 2) == 0;
                g.dispersal = nudge(g.dispersal, DISPERSAL_MAX, up);
            }
            _ => {
                let up = rng.below(tick, Purpose::BehaviorValue, subject, 2) == 0;
                g.boldness = nudge(g.boldness, BOLDNESS_MAX, up);
            }
        }
    }
    if rng.chance_ppm(tick, Purpose::HueChance, subject, rules.hue_mutation_ppm) {
        let r = rng.below(tick, Purpose::HueStep, subject, 16) as u16;
        let delta = 1 + r / 2; // 1..=8
        g.hue = if r.is_multiple_of(2) {
            (g.hue + delta) % HUE_RANGE
        } else {
            (g.hue + HUE_RANGE - delta) % HUE_RANGE
        };
    }
    if g.gifts != 0 {
        if let Some(p) = &rules.patrons {
            for (k, &gift) in GIFTS.iter().enumerate() {
                if g.has(gift)
                    && rng.raw(tick, Purpose::GiftLoss, subject, k as u32) % 1_000_000
                        < u64::from(p.gift_loss_ppm)
                {
                    g.gifts &= !gift;
                }
            }
        }
    }
    debug_assert!(g.is_valid(rules.trait_budget));
    g
}

/// Moves a value one step up or down, bouncing off the ends of `0..=max`.
fn nudge(value: u8, max: u8, up: bool) -> u8 {
    match (up, value) {
        (true, v) if v < max => v + 1,
        (true, v) => v - 1,
        (false, 0) => 1,
        (false, v) => v - 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::derive;

    const SAMPLE: Genome = Genome {
        traits: [2, 3, 8, 0, 4, 7],
        habitat: 0,
        dispersal: 1,
        boldness: 3,
        hue: 355,
        gifts: 0,
        extra: [0, 0],
    };

    #[test]
    fn sample_is_valid() {
        assert!(SAMPLE.is_valid(24));
        assert_eq!(SAMPLE.steps(), 1);
        assert_eq!(SAMPLE.sight(), 2);
    }

    #[test]
    fn distance_counts_mutation_steps() {
        let mut other = SAMPLE;
        other.traits[2] -= 1;
        other.traits[3] += 1;
        assert_eq!(SAMPLE.distance(&other), 1);
        assert_eq!(other.distance(&SAMPLE), 1);
        other.hue = 10;
        other.habitat = 4;
        assert_eq!(SAMPLE.distance(&other), 1);
    }

    #[test]
    fn mutations_keep_genomes_valid() {
        let rules = Ruleset {
            mutation_ppm: 1_000_000,
            behavior_mutation_ppm: 1_000_000,
            hue_mutation_ppm: 1_000_000,
            ..Ruleset::default()
        };
        let rng = Rng::new(&derive(b"mutations", &[]));
        let mut g = SAMPLE;
        for subject in 0..5000 {
            let child = mutate(&g, &rules, &rng, 0, subject);
            assert!(child.is_valid(rules.trait_budget), "{child:?}");
            assert!(g.distance(&child) <= 1);
            g = child;
        }
    }

    #[test]
    fn nudge_bounces() {
        assert_eq!(nudge(3, 3, true), 2);
        assert_eq!(nudge(0, 3, false), 1);
        assert_eq!(nudge(1, 3, true), 2);
        assert_eq!(nudge(1, 3, false), 0);
    }
}
