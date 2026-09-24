//! Season rules (spec Part II, `docs/ruleset.md`).
//!
//! Stage A1 holds the parameters the core uses. The default values are untuned candidates;
//! the balance harness exists to replace them.

use serde::{Deserialize, Serialize};

use crate::genome::TRAIT_MAX;
use crate::rng::derive;
use crate::state::Biome;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiomeParams {
    pub passable: bool,
    /// Food growth per tick, in food units.
    pub base_regen: u32,
    pub food_max: u32,
    /// The energy cost of stepping into a cell of this biome, in hundredths.
    pub move_cost: i32,
    /// Cover: a defense bonus for organisms standing in this biome. Stage A1 experiment,
    /// off (0) by default and not yet in the specification.
    #[serde(default)]
    pub cover: i32,
}

/// Weights of the movement score (spec §11.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveWeights {
    /// Percentage applied to the expected energy from food.
    pub food_pct: i32,
    /// Score per point of attack margin over the weakest non-kin organism in a cell.
    pub hunt_per_point: i32,
    /// Score per point of the strongest nearby hunter's margin, scaled by `(4 − boldness) / 4`.
    pub danger_per_point: i32,
    /// Bonus for the preferred biome.
    pub habitat_bonus: i32,
    /// Penalty per other organism in the cell.
    pub crowd_per_neighbor: i32,
    /// Bonus per step of distance, multiplied by `dispersal`.
    pub dispersal_per_step: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ruleset {
    pub version: u32,
    pub width: u16,
    pub height: u16,
    pub ticks_per_epoch: u32,
    pub epochs_per_day: u32,
    pub max_organisms: u32,
    pub max_per_cell: u8,
    pub founder_lineages: u32,
    pub organisms_per_lineage: u32,
    /// The founders' energy, in hundredths.
    pub genesis_energy: i32,
    pub land_share_pct: u32,
    /// Indexed by `Biome as usize`.
    pub biomes: [BiomeParams; Biome::COUNT],
    /// Energy values are in hundredths of a unit.
    pub energy_max: i32,
    pub base_metabolism: i32,
    pub trait_upkeep: [i32; 6],
    pub habitat_modifier_pct: i32,
    pub shallow_drain: i32,
    pub bite_per_point: u32,
    /// Energy (hundredths) gained per food unit eaten.
    pub plant_efficiency: i32,
    pub attack_weight: i32,
    pub defense_weight: i32,
    pub roll_span: u32,
    pub attack_cost: i32,
    /// Satiation: an organism hunts only while its energy is below this share of
    /// `energy_max`. Stage A1 addition, not yet in the specification.
    pub hunt_hunger_pct: i32,
    pub predation_efficiency_pct: i32,
    pub body_value: i32,
    pub kin_distance: u32,
    pub repro_base: i32,
    pub repro_per_fertility: i32,
    pub child_base: i32,
    pub child_per_fertility: i32,
    pub birth_cost: i32,
    pub senescence_start: u32,
    pub max_age: u32,
    /// Detritus left by a body, in food units.
    pub body_detritus: u32,
    /// Detritus left by a predator's kill, in food units.
    pub remains_detritus: u32,
    pub decomposition_pct: u32,
    /// Mutation probabilities per birth, in parts per million.
    pub mutation_ppm: u32,
    pub behavior_mutation_ppm: u32,
    pub hue_mutation_ppm: u32,
    pub clade_split_distance: u32,
    pub clade_name_threshold: u32,
    pub weights: MoveWeights,
}

impl Default for Ruleset {
    fn default() -> Self {
        let biome = |passable, base_regen, food_max, move_cost| BiomeParams {
            passable,
            base_regen,
            food_max,
            move_cost,
            cover: 0,
        };
        Self {
            version: 0,
            width: 64,
            height: 64,
            ticks_per_epoch: 12,
            epochs_per_day: 288,
            max_organisms: 6000,
            max_per_cell: 4,
            founder_lineages: 6,
            organisms_per_lineage: 50,
            genesis_energy: 8000,
            land_share_pct: 60,
            biomes: [
                biome(false, 0, 0, 0),   // deep water
                biome(true, 0, 0, 90),   // shallows
                biome(true, 12, 60, 30), // forest
                biome(true, 9, 45, 20),  // steppe
                biome(true, 3, 15, 30),  // desert
                biome(true, 4, 20, 80),  // mountains
                biome(true, 10, 50, 60), // swamp
            ],
            energy_max: 20_000,
            base_metabolism: 100,
            trait_upkeep: [15, 10, 8, 15, 12, 8],
            habitat_modifier_pct: 10,
            shallow_drain: 200,
            bite_per_point: 3,
            plant_efficiency: 50,
            attack_weight: 3,
            defense_weight: 4,
            roll_span: 8,
            attack_cost: 150,
            hunt_hunger_pct: 60,
            predation_efficiency_pct: 60,
            body_value: 2000,
            kin_distance: 2,
            repro_base: 12_000,
            repro_per_fertility: 600,
            child_base: 5000,
            child_per_fertility: 300,
            birth_cost: 500,
            senescence_start: 150,
            max_age: 300,
            body_detritus: 10,
            remains_detritus: 3,
            decomposition_pct: 5,
            mutation_ppm: 100_000,
            behavior_mutation_ppm: 50_000,
            hue_mutation_ppm: 500_000,
            clade_split_distance: 3,
            clade_name_threshold: 20,
            weights: MoveWeights {
                food_pct: 100,
                hunt_per_point: 100,
                danger_per_point: 100,
                habitat_bonus: 200,
                crowd_per_neighbor: 150,
                dispersal_per_step: 10,
            },
        }
    }
}

impl Ruleset {
    /// `BLAKE3("PROTOGAEA/RULESET/V0" ‖ JSON)`. Stage A1 uses serde's field order as the
    /// canonical encoding; the final canonicalization is TBD (`docs/ruleset.md`).
    pub fn ruleset_id(&self) -> [u8; 32] {
        let json = serde_json::to_vec(self).expect("a ruleset always serializes");
        derive(b"PROTOGAEA/RULESET/V0", &[&json])
    }

    /// Rejects parameter sets the core cannot run safely.
    pub fn validate(&self) -> Result<(), String> {
        let cells = u32::from(self.width) * u32::from(self.height);
        if cells == 0 || cells > u32::from(u16::MAX) + 1 {
            return Err("the map must have between 1 and 65,536 cells".into());
        }
        if !(1..=4).contains(&self.max_per_cell) {
            return Err("max_per_cell must be between 1 and 4".into());
        }
        if self.ticks_per_epoch == 0 || self.epochs_per_day == 0 {
            return Err("ticks_per_epoch and epochs_per_day must be positive".into());
        }
        if self.max_age <= self.senescence_start {
            return Err("max_age must be greater than senescence_start".into());
        }
        if self.land_share_pct == 0 || self.land_share_pct > 100 {
            return Err("land_share_pct must be between 1 and 100".into());
        }
        if self.biomes.iter().any(|b| b.cover < 0) {
            return Err("cover must not be negative".into());
        }
        if self.energy_max <= 0 || self.genesis_energy <= 0 || self.genesis_energy > self.energy_max
        {
            return Err("genesis_energy must be positive and at most energy_max".into());
        }
        if self.decomposition_pct > 100
            || !(0..=100).contains(&self.habitat_modifier_pct)
            || !(0..=100).contains(&self.hunt_hunger_pct)
        {
            return Err("percentages must be between 0 and 100".into());
        }
        for fertility in 0..=i32::from(TRAIT_MAX) {
            let threshold = self.repro_base - fertility * self.repro_per_fertility;
            let child = self.child_base - fertility * self.child_per_fertility;
            if child <= 0 || threshold <= child + self.birth_cost || threshold > self.energy_max {
                return Err(format!(
                    "reproduction at fertility {fertility} is inconsistent: the threshold must be \
                     at most energy_max and above the child's energy plus birth_cost"
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_valid() {
        Ruleset::default().validate().unwrap();
    }

    #[test]
    fn ruleset_id_changes_with_parameters() {
        let a = Ruleset::default();
        let b = Ruleset {
            mutation_ppm: 1,
            ..Ruleset::default()
        };
        assert_eq!(a.ruleset_id(), Ruleset::default().ruleset_id());
        assert_ne!(a.ruleset_id(), b.ruleset_id());
    }

    #[test]
    fn json_round_trip() {
        let rules = Ruleset::default();
        let json = serde_json::to_string(&rules).unwrap();
        assert_eq!(serde_json::from_str::<Ruleset>(&json).unwrap(), rules);
    }
}
