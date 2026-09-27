//! Season rules (spec Part II, `docs/ruleset.md`).
//!
//! The default values are untuned candidates; the balance harness exists to replace them.
//! Food is counted in tenths of a unit, so that integer multipliers do not round slow growth
//! down to zero.

use serde::{Deserialize, Serialize};

use crate::genome::{Genome, TRAIT_BUDGET_MAX, TRAIT_MAX};
use crate::rng::derive;
use crate::state::Biome;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiomeParams {
    pub passable: bool,
    /// Food growth per tick before multipliers, in tenths of a food unit.
    pub base_regen: u32,
    pub food_max: u32,
    /// The energy cost of stepping into a cell of this biome, in hundredths.
    pub move_cost: i32,
    /// Cover: a defense bonus for organisms standing in this biome (spec §11.3).
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

/// A founder lineage (spec §9).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Founder {
    pub genome: Genome,
    pub biome: Biome,
}

fn default_trait_budget() -> u32 {
    24
}

/// Four characters for the future continents: every one keeps all five land biomes, but the
/// proportions differ, which gives more niches and more clades (harness findings, stage A3).
fn default_plate_mixes() -> Vec<BiomeMix> {
    let mix = |mountains_pct, desert_pct, steppe_pct, forest_pct| BiomeMix {
        mountains_pct,
        desert_pct,
        steppe_pct,
        forest_pct,
    };
    vec![
        // Arid steppe: open land, sparse food.
        mix(10, 32, 44, 16),
        // Forest: cover everywhere.
        mix(10, 6, 18, 56),
        // Highland.
        mix(28, 16, 30, 36),
        // Wetland.
        mix(8, 8, 22, 34),
    ]
}

/// Six archetypes in different biomes.
fn default_founders() -> Vec<Founder> {
    let founder = |traits, habitat, dispersal, boldness, hue, biome| Founder {
        genome: Genome {
            traits,
            habitat,
            dispersal,
            boldness,
            hue,
            gifts: 0,
            extra: [0, 0],
        },
        biome,
    };
    vec![
        // Grazer: eats well, breeds fast, easy prey.
        founder([2, 3, 8, 0, 4, 7], 0, 1, 1, 120, Biome::Forest),
        // Hunter.
        founder([6, 6, 0, 8, 2, 2], 1, 2, 3, 0, Biome::Steppe),
        // Armored: too hard for hunters, but a slower grazer.
        founder([1, 2, 6, 0, 8, 7], 3, 0, 2, 215, Biome::Mountains),
        // Forager: quick and sharp-eyed.
        founder([5, 5, 6, 0, 3, 5], 2, 2, 1, 40, Biome::Desert),
        // Breeder.
        founder([3, 3, 7, 0, 3, 8], 4, 1, 0, 285, Biome::Swamp),
        // Generalist omnivore.
        founder([4, 4, 4, 4, 4, 4], 5, 1, 2, 170, Biome::Forest),
    ]
}

/// The ten founders of spec v0.3 (draft): the six of v0.2 with size and longevity, and four new
/// ones for the new niches. Traits: movement, perception, plants, hunting, defense, fertility; then
/// size and longevity; all eight sum to 32.
fn v03_founders() -> Vec<Founder> {
    let founder = |traits, extra, gifts, habitat, dispersal, boldness, hue, biome| Founder {
        genome: Genome {
            traits,
            habitat,
            dispersal,
            boldness,
            hue,
            gifts,
            extra,
        },
        biome,
    };
    use crate::genome::SWIM;
    vec![
        founder([2, 3, 8, 0, 4, 7], [3, 5], 0, 0, 1, 1, 120, Biome::Forest),
        founder([6, 6, 0, 8, 2, 2], [5, 3], 0, 1, 2, 3, 0, Biome::Steppe),
        founder(
            [1, 2, 6, 0, 8, 7],
            [5, 3],
            0,
            3,
            0,
            2,
            215,
            Biome::Mountains,
        ),
        founder([5, 5, 6, 0, 3, 5], [3, 5], 0, 2, 2, 1, 40, Biome::Desert),
        founder([3, 3, 7, 0, 3, 8], [4, 4], 0, 4, 1, 0, 285, Biome::Swamp),
        founder([4, 4, 4, 4, 4, 4], [4, 4], 0, 5, 1, 2, 170, Biome::Forest),
        // Scavenger: eats plants and detritus, hunts a little.
        founder([4, 4, 6, 3, 3, 4], [4, 4], 0, 1, 1, 2, 75, Biome::Steppe),
        // Coastal swimmer: grazes the algae of the shallows.
        founder([4, 3, 7, 0, 3, 7], [3, 5], SWIM, 4, 2, 1, 190, Biome::Swamp),
        // Burrower: slow and armored, hides from hunters.
        founder([1, 4, 6, 0, 7, 6], [4, 4], 0, 2, 0, 1, 25, Biome::Desert),
        // Giant: huge, slow to breed, hard to kill.
        founder([2, 2, 8, 2, 6, 2], [8, 2], 0, 1, 0, 3, 330, Biome::Steppe),
    ]
}

impl Ruleset {
    /// The rules of spec v0.3 (draft): patrons, eight traits with a budget of 32, the ten
    /// founders at 40 each, and algae in the shallows for swimmers.
    pub fn v03() -> Ruleset {
        let mut r = Ruleset {
            patrons: Some(Patrons::default()),
            traits8: Some(Traits8::default()),
            trait_budget: 32,
            founders: v03_founders(),
            organisms_per_lineage: 40,
            // Eight traits give more ways to drift apart: a clade splits a step later.
            clade_split_distance: 4,
            ..Ruleset::default()
        };
        r.biomes[Biome::Shallows as usize] = BiomeParams {
            passable: true,
            base_regen: 35,
            food_max: 120,
            move_cost: 90,
            cover: 0,
        };
        r
    }
}

/// Times of year and moisture (spec §10). Times of year are, in order: spring, summer,
/// autumn, winter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Climate {
    /// Epochs in a world year, divisible by 4. It should not be a multiple of a world day.
    pub epochs_per_year: u32,
    /// Food growth multiplier per biome and time of year, in percent.
    pub season_mult: [[u32; 4]; Biome::COUNT],
    /// Base moisture of each biome, 0–100.
    pub moisture_base: [u8; Biome::COUNT],
    /// Seasonal shift of the base moisture (spec §10).
    pub season_moisture_delta: [i32; 4],
    /// How far moisture moves toward its base per tick.
    pub moisture_relax: u8,
    /// Food growth at zero moisture and at full moisture, in percent; linear in between.
    pub moisture_mult_min_pct: u32,
    pub moisture_mult_max_pct: u32,
    /// Cold: extra energy cost of every organism in the middle of winter at the northern
    /// edge of the map, in percent. It follows the times of year like `season_mult` and falls
    /// linearly to zero at the southern edge, so that plates at different latitudes differ.
    #[serde(default)]
    pub cold_winter_pct: u32,
}

/// Natural events, drawn at the epoch boundary from the epoch seed (spec §10).
/// Probabilities are in parts per million per epoch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Events {
    /// At most this many active effects of each kind.
    pub max_active_per_kind: u32,
    /// Wildfire: forest or steppe, in summer, where moisture is below `wildfire_max_moisture`.
    pub wildfire_ppm: u32,
    pub wildfire_max_moisture: u8,
    pub wildfire_radius_min: u8,
    pub wildfire_radius_max: u8,
    pub wildfire_energy_loss_pct: i32,
    /// After a wildfire, ash raises food growth for a while.
    pub ash_growth_pct: u32,
    pub ash_ticks: u32,
    /// Great drought: steppe or desert, in summer; a square of side `2 × radius + 1`.
    pub drought_ppm: u32,
    pub drought_radius: u8,
    pub drought_growth_pct: u32,
    pub drought_moisture_drop: u8,
    pub drought_ticks: u32,
    /// Flood: swamps and land next to water, in spring. Land other than mountains within
    /// `flood_radius` acts as shallows for `flood_ticks`; its food is lost and its soil soaked.
    pub flood_ppm: u32,
    pub flood_radius: u8,
    pub flood_ticks: u32,
    /// Plague ("kill the winner"): strikes the most numerous clade once its share of the
    /// population exceeds `plague_min_share_permille`. The chance grows linearly with the
    /// share, up to `plague_max_ppm` at 100%.
    pub plague_min_share_permille: u32,
    pub plague_max_ppm: u32,
    pub plague_radius: u8,
    /// No plague unless at least this many members of the clade are within the radius.
    pub plague_min_members: u32,
    pub plague_mortality_ppm: u32,
}

/// The Breaking of Pangea (spec §4, §10). Plate boundaries turn into rifts on a schedule and
/// split the continent. Days are world days from genesis.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rifts {
    /// The number of plates, the future continents, is drawn from this range at genesis.
    /// Zero means no rifts.
    pub plates_min: u8,
    pub plates_max: u8,
    /// How far plate boundaries wander from straight lines, in cells.
    pub boundary_warp: u8,
    /// Rift cells become faults on this day.
    pub fault_day: u32,
    /// Rift cells turn into shallows in waves from the ocean inward, from this day until
    /// `deep_from_day`.
    pub shallows_from_day: u32,
    /// The shallows deepen into deep water in the same waves, until `bridges_from_day`.
    pub deep_from_day: u32,
    /// Land bridges close one by one between these days.
    pub bridges_from_day: u32,
    pub bridges_to_day: u32,
    /// A fault's food growth and step cost, in percent of its biome's.
    pub fault_growth_pct: u32,
    pub fault_move_pct: i32,
    /// A land bridge is every rift cell within this Chebyshev distance of its center.
    pub bridge_radius: u8,
    /// How far an organism can be carried from a sinking cell to free land; beyond it drowns.
    pub rescue_radius: u8,
    /// Biome mixes of the future continents, so that their ecologies differ. At genesis the
    /// plates take them in turn from a random starting point. Empty: every plate uses
    /// `Ruleset::biome_mix`. A ruleset file without this field keeps the empty list.
    #[serde(default)]
    pub plate_mixes: Vec<BiomeMix>,
}

/// The shares of land biomes (spec §4, §9). Mountains take the highest land by height; the
/// rest of the land is divided by moisture, from dry to wet: desert, steppe, forest, and swamp
/// for whatever remains. Every share must be positive, so that every biome appears.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiomeMix {
    /// Percent of the land.
    pub mountains_pct: u8,
    /// Percent of the land other than mountains.
    pub desert_pct: u8,
    pub steppe_pct: u8,
    pub forest_pct: u8,
}

impl Default for BiomeMix {
    /// 12% mountains; the rest 20% desert, 30% steppe, 35% forest and 15% swamp.
    fn default() -> Self {
        Self {
            mountains_pct: 12,
            desert_pct: 20,
            steppe_pct: 30,
            forest_pct: 35,
        }
    }
}

impl BiomeMix {
    fn is_valid(&self) -> bool {
        let rest =
            u32::from(self.desert_pct) + u32::from(self.steppe_pct) + u32::from(self.forest_pct);
        (1..100).contains(&self.mountains_pct)
            && self.desert_pct > 0
            && self.steppe_pct > 0
            && self.forest_pct > 0
            && rest < 100
    }
}

/// Miracles (spec §5): the effects and limits of `weather`, `migrate` and `revive`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Miracles {
    /// `weather` covers the square of this radius (3: 7 × 7) for this many ticks.
    pub weather_radius: u8,
    pub weather_ticks: u32,
    /// Food growth under rain and drought, in percent.
    pub rain_growth_pct: u32,
    pub dry_growth_pct: u32,
    /// Moisture added by rain or taken by drought when the miracle begins.
    pub weather_moisture: u8,
    /// A weather region and a relocated clade cool down for this many epochs.
    pub cooldown_epochs: u32,
    /// `migrate`: the source area's radius (2: 5 × 5), the clade's members needed there, and how
    /// many move.
    pub migrate_radius: u8,
    pub migrate_min: u32,
    pub migrate_moved: u32,
    /// `revive`: how long a museum entry must have been extinct, the radius (2: 5 × 5) and the
    /// most organisms around the start, and how many organisms come back.
    pub revive_extinct_epochs: u32,
    pub revive_radius: u8,
    pub revive_max_nearby: u32,
    pub revive_count: u32,
}

impl Default for Miracles {
    fn default() -> Self {
        Self {
            weather_radius: 3,
            weather_ticks: 36,
            rain_growth_pct: 150,
            dry_growth_pct: 50,
            weather_moisture: 30,
            cooldown_epochs: 12,
            migrate_radius: 2,
            migrate_min: 10,
            migrate_moved: 3,
            revive_extinct_epochs: 36,
            revive_radius: 2,
            revive_max_nearby: 8,
            revive_count: 5,
        }
    }
}

/// Patrons of clades (spec v0.3, draft): help for a clade (easing, gifts), harm to a rival, and
/// weather as a natural event. A ruleset without this section plays as v0.2.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Patrons {
    /// Easing and harm cover the square of this radius (3: 7 × 7) around their center.
    pub area_radius: u8,
    /// A clade's members counted around the center (2: 5 × 5), and how many are needed there.
    pub target_radius: u8,
    pub min_members: u32,
    /// How long easing and harm last, and `cure`; a clade is cured at most once per
    /// `cure_cooldown_epochs`.
    pub effect_ticks: u32,
    pub cure_ticks: u32,
    pub cure_cooldown_epochs: u32,
    /// `shelter` adds to the clade's defense, `expose` takes from it.
    pub shelter_defense: i32,
    pub expose_defense: i32,
    /// `forage` lowers the clade's metabolism, `blight` raises it, in percent.
    pub forage_metabolism_pct: i32,
    pub blight_metabolism_pct: i32,
    /// `sickness`: the chance per tick that a member of the clade in the area dies.
    pub sickness_ppm: u32,
    /// Harm is refused against a clade with fewer members than this or a smaller share of the
    /// living (per mille); after harm, a clade rests from harm for `harm_respite_epochs`.
    pub protect_min_living: u32,
    pub protect_min_permille: u32,
    pub harm_respite_epochs: u32,
    /// A gift goes to this many members around the center; a clade gets at most one gift per
    /// `gift_cooldown_epochs`. A newborn loses each gift with `gift_loss_ppm`; an organism holds
    /// at most `max_gifts`.
    pub gift_count: u32,
    pub gift_cooldown_epochs: u32,
    pub gift_loss_ppm: u32,
    pub max_gifts: u32,
    /// Energy per tick for each gift held, in the order of `genome::GIFTS`.
    pub gift_upkeep: [i32; 6],
    /// `venom`, `camo` and `keen` (which sees danger coming) add to defense; a hunter that kills
    /// a venomous organism loses this share of its energy.
    pub venom_defense: i32,
    pub venom_drain_pct: i32,
    pub camo_defense: i32,
    pub keen_defense: i32,
    /// `scavenge`: detritus eaten is worth this share of food, in percent.
    pub scavenge_value_pct: i32,
    /// Natural weather: the map is split into regions, and each gets rain or drought with this
    /// chance per epoch (the effect of the v0.2 `weather` miracle).
    pub weather_regions_x: u8,
    pub weather_regions_y: u8,
    pub weather_ppm: u32,
}

impl Default for Patrons {
    fn default() -> Self {
        Self {
            area_radius: 3,
            target_radius: 2,
            min_members: 10,
            effect_ticks: 36,
            cure_ticks: 72,
            cure_cooldown_epochs: 36,
            shelter_defense: 10,
            expose_defense: 10,
            forage_metabolism_pct: 20,
            blight_metabolism_pct: 20,
            sickness_ppm: 10_000,
            protect_min_living: 20,
            protect_min_permille: 30,
            harm_respite_epochs: 24,
            gift_count: 10,
            gift_cooldown_epochs: 36,
            gift_loss_ppm: 5_000,
            max_gifts: 2,
            gift_upkeep: [10, 10, 10, 2, 5, 15],
            venom_defense: 6,
            venom_drain_pct: 30,
            camo_defense: 6,
            keen_defense: 4,
            scavenge_value_pct: 33,
            weather_regions_x: 4,
            weather_regions_y: 2,
            weather_ppm: 30_000,
        }
    }
}

/// Size, longevity and the niches of spec v0.3 (draft). A ruleset without this section plays with
/// the six traits of v0.2.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Traits8 {
    /// The energy an organism holds: `energy_max × (base + per point of size) / 100`.
    pub size_energy_base_pct: i32,
    pub size_energy_pct: i32,
    /// Per point of size: attack, defense, energy per tick, and the energy to breed and the
    /// newborn's, both higher.
    pub size_attack: i32,
    pub size_defense: i32,
    pub size_upkeep: i32,
    pub size_repro: i32,
    pub size_child: i32,
    /// Aging starts and death comes at `(base + per point of longevity)%` of the rules' ages;
    /// longevity costs energy per tick.
    pub longevity_base_pct: u32,
    pub longevity_pct: u32,
    pub longevity_upkeep: i32,
    /// Burrowers: an organism this slow and this armored hides when a hunter is near, skipping
    /// the tick with extra defense.
    pub burrow_max_movement: u8,
    pub burrow_min_defense: u8,
    pub burrow_defense: i32,
    /// Scavengers by birth: plant eating and hunting at least these eat detritus, worth this
    /// share of food.
    pub scavenger_min_plants: u8,
    pub scavenger_min_hunting: u8,
    pub scavenger_value_pct: i32,
}

impl Default for Traits8 {
    fn default() -> Self {
        Self {
            size_energy_base_pct: 70,
            size_energy_pct: 11,
            size_attack: 1,
            size_defense: 1,
            size_upkeep: 6,
            size_repro: 300,
            size_child: 150,
            longevity_base_pct: 80,
            longevity_pct: 7,
            longevity_upkeep: 8,
            burrow_max_movement: 3,
            burrow_min_defense: 5,
            burrow_defense: 25,
            scavenger_min_plants: 6,
            scavenger_min_hunting: 3,
            scavenger_value_pct: 50,
        }
    }
}

/// Natural revival from the spore bank and the end of a season by extinction (spec §12).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revival {
    /// The spore bank revives the world when fewer organisms than this are alive.
    pub below: u32,
    /// Organisms placed per genome in the spore bank.
    pub per_genome: u32,
    /// At most one revival per this many epochs.
    pub cooldown_epochs: u32,
    /// This many revivals within `season_end_days` end the season by extinction.
    pub season_end_count: u32,
    pub season_end_days: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ruleset {
    pub version: u32,
    pub width: u16,
    pub height: u16,
    pub ticks_per_epoch: u32,
    pub epochs_per_day: u32,
    /// The length of a season, in world days (spec §4).
    pub season_days: u32,
    pub max_organisms: u32,
    pub max_per_cell: u8,
    /// The sum of the six traits of every genome (spec §11.1): one trait grows only at
    /// another's expense.
    #[serde(default = "default_trait_budget")]
    pub trait_budget: u32,
    /// Founder lineages: a genome and the biome it starts in (spec §9).
    pub founders: Vec<Founder>,
    pub organisms_per_lineage: u32,
    /// The founders' energy, in hundredths.
    pub genesis_energy: i32,
    pub land_share_pct: u32,
    /// The biome mix of the land, unless the plates have their own (`Rifts::plate_mixes`).
    #[serde(default)]
    pub biome_mix: BiomeMix,
    /// Indexed by `Biome as usize`.
    pub biomes: [BiomeParams; Biome::COUNT],
    pub climate: Climate,
    pub events: Events,
    pub rifts: Rifts,
    /// Energy values are in hundredths of a unit.
    pub energy_max: i32,
    pub base_metabolism: i32,
    pub trait_upkeep: [i32; 6],
    pub habitat_modifier_pct: i32,
    pub shallow_drain: i32,
    /// Food (tenths) eaten per point of plant eating, per tick.
    pub bite_per_point: u32,
    /// Energy (hundredths) gained per tenth of a food unit eaten.
    pub plant_efficiency: i32,
    pub attack_weight: i32,
    pub defense_weight: i32,
    pub roll_span: u32,
    pub attack_cost: i32,
    /// Satiation: an organism hunts only while its energy is below this share of
    /// `energy_max` (spec §11.3).
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
    /// Detritus left by a body, in tenths of a food unit.
    pub body_detritus: u32,
    /// Detritus left by a predator's kill, in tenths of a food unit.
    pub remains_detritus: u32,
    pub decomposition_pct: u32,
    /// Mutation probabilities per birth, in parts per million.
    pub mutation_ppm: u32,
    pub behavior_mutation_ppm: u32,
    pub hue_mutation_ppm: u32,
    pub clade_split_distance: u32,
    pub clade_name_threshold: u32,
    /// Extinct named clades kept in the state (spec §12).
    pub museum_capacity: u32,
    pub revival: Revival,
    pub weights: MoveWeights,
    #[serde(default)]
    pub miracles: Miracles,
    /// Patrons of clades (spec v0.3, draft). Absent, the world plays as v0.2, and the ruleset id
    /// stays what it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patrons: Option<Patrons>,
    /// Size, longevity and the new niches (spec v0.3, draft). Absent, genomes have six traits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traits8: Option<Traits8>,
}

impl Default for Ruleset {
    fn default() -> Self {
        let biome = |passable, base_regen, food_max, move_cost, cover| BiomeParams {
            passable,
            base_regen,
            food_max,
            move_cost,
            cover,
        };
        Self {
            version: 0,
            width: 64,
            height: 64,
            ticks_per_epoch: 12,
            epochs_per_day: 288,
            season_days: 42,
            max_organisms: 6000,
            max_per_cell: 4,
            trait_budget: default_trait_budget(),
            founders: default_founders(),
            organisms_per_lineage: 50,
            genesis_energy: 8000,
            land_share_pct: 60,
            biome_mix: BiomeMix::default(),
            biomes: [
                biome(false, 0, 0, 0, 0),      // deep water
                biome(true, 0, 0, 90, 0),      // shallows
                biome(true, 170, 600, 30, 12), // forest
                biome(true, 130, 450, 20, 0),  // steppe
                biome(true, 45, 150, 30, 0),   // desert
                biome(true, 60, 200, 80, 14),  // mountains
                biome(true, 140, 500, 60, 10), // swamp
            ],
            climate: Climate {
                epochs_per_year: 372,
                season_mult: [
                    [100, 100, 100, 100], // deep water
                    [100, 100, 100, 100], // shallows
                    [110, 120, 90, 40],   // forest
                    [120, 90, 80, 30],    // steppe
                    [80, 40, 70, 60],     // desert
                    [70, 100, 60, 20],    // mountains
                    [120, 110, 100, 50],  // swamp
                ],
                moisture_base: [100, 100, 70, 45, 15, 50, 90],
                season_moisture_delta: [10, -20, 0, 10],
                moisture_relax: 1,
                moisture_mult_min_pct: 50,
                moisture_mult_max_pct: 110,
                cold_winter_pct: 0,
            },
            events: Events {
                max_active_per_kind: 2,
                wildfire_ppm: 9000,
                wildfire_max_moisture: 30,
                wildfire_radius_min: 2,
                wildfire_radius_max: 4,
                wildfire_energy_loss_pct: 50,
                ash_growth_pct: 150,
                ash_ticks: 72,
                drought_ppm: 4630,
                drought_radius: 4,
                drought_growth_pct: 50,
                drought_moisture_drop: 30,
                drought_ticks: 72,
                // About once a day in spring.
                flood_ppm: 3472,
                flood_radius: 2,
                flood_ticks: 24,
                plague_min_share_permille: 300,
                plague_max_ppm: 150_000,
                plague_radius: 3,
                plague_min_members: 8,
                plague_mortality_ppm: 500_000,
            },
            rifts: Rifts {
                plates_min: 3,
                plates_max: 4,
                boundary_warp: 5,
                fault_day: 7,
                shallows_from_day: 14,
                deep_from_day: 24,
                bridges_from_day: 35,
                bridges_to_day: 39,
                fault_growth_pct: 50,
                fault_move_pct: 200,
                bridge_radius: 2,
                rescue_radius: 8,
                plate_mixes: default_plate_mixes(),
            },
            energy_max: 20_000,
            base_metabolism: 100,
            trait_upkeep: [15, 10, 8, 10, 12, 8],
            habitat_modifier_pct: 10,
            shallow_drain: 200,
            bite_per_point: 30,
            plant_efficiency: 5,
            attack_weight: 3,
            defense_weight: 4,
            roll_span: 8,
            attack_cost: 150,
            hunt_hunger_pct: 70,
            predation_efficiency_pct: 60,
            body_value: 2000,
            kin_distance: 1,
            repro_base: 12_000,
            repro_per_fertility: 600,
            child_base: 5000,
            child_per_fertility: 300,
            birth_cost: 500,
            senescence_start: 150,
            max_age: 300,
            body_detritus: 100,
            remains_detritus: 30,
            decomposition_pct: 5,
            mutation_ppm: 140_000,
            behavior_mutation_ppm: 50_000,
            hue_mutation_ppm: 500_000,
            clade_split_distance: 3,
            clade_name_threshold: 20,
            museum_capacity: 1024,
            revival: Revival {
                below: 30,
                per_genome: 5,
                cooldown_epochs: 288,
                season_end_count: 3,
                season_end_days: 7,
            },
            weights: MoveWeights {
                food_pct: 100,
                hunt_per_point: 100,
                danger_per_point: 100,
                habitat_bonus: 300,
                crowd_per_neighbor: 150,
                dispersal_per_step: 10,
            },
            miracles: Miracles::default(),
            patrons: None,
            traits8: None,
        }
    }
}

impl Ruleset {
    /// `BLAKE3("PROTOGAEA/RULESET/V0" ‖ JSON)`. The canonical encoding is serde's field order
    /// for now; the final canonicalization is TBD (`docs/ruleset.md`).
    pub fn ruleset_id(&self) -> [u8; 32] {
        let json = serde_json::to_vec(self).expect("a ruleset always serializes");
        derive(b"PROTOGAEA/RULESET/V0", &[&json])
    }

    /// Ticks in a world year.
    pub fn year_ticks(&self) -> u64 {
        u64::from(self.climate.epochs_per_year) * u64::from(self.ticks_per_epoch)
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
        if self.climate.epochs_per_year == 0 || !self.climate.epochs_per_year.is_multiple_of(4) {
            return Err("epochs_per_year must be a positive multiple of 4".into());
        }
        if self.max_age <= self.senescence_start {
            return Err("max_age must be greater than senescence_start".into());
        }
        if self.land_share_pct == 0 || self.land_share_pct > 100 {
            return Err("land_share_pct must be between 1 and 100".into());
        }
        if !(1..=TRAIT_BUDGET_MAX).contains(&self.trait_budget) {
            return Err(format!(
                "trait_budget must be between 1 and {TRAIT_BUDGET_MAX}"
            ));
        }
        if self.founders.is_empty()
            || self
                .founders
                .iter()
                .any(|f| !f.genome.is_valid(self.trait_budget) || !f.biome.is_land())
        {
            return Err(
                "founders must be valid genomes that spend the trait budget, on land biomes".into(),
            );
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
            || !(0..=100).contains(&self.events.wildfire_energy_loss_pct)
        {
            return Err("percentages must be between 0 and 100".into());
        }
        let c = &self.climate;
        if c.moisture_base.iter().any(|&m| m > 100)
            || c.moisture_mult_min_pct > c.moisture_mult_max_pct
            || c.season_mult.iter().flatten().any(|&m| m > 1000)
            || c.cold_winter_pct > 1000
        {
            return Err("climate parameters are out of range".into());
        }
        let e = &self.events;
        if e.wildfire_radius_min > e.wildfire_radius_max
            || e.plague_min_share_permille >= 1000
            || e.plague_mortality_ppm > 1_000_000
            || e.flood_radius > 16
        {
            return Err("event parameters are out of range".into());
        }
        let r = &self.rifts;
        if r.plates_max > 8 || r.plates_min > r.plates_max || (r.plates_max > 0 && r.plates_min < 2)
        {
            return Err("rifts need between 2 and 8 plates, or none".into());
        }
        if !(r.fault_day <= r.shallows_from_day
            && r.shallows_from_day <= r.deep_from_day
            && r.deep_from_day <= r.bridges_from_day
            && r.bridges_from_day <= r.bridges_to_day)
        {
            return Err("rift phases must follow in order".into());
        }
        if r.fault_growth_pct > 1000 || !(0..=1000).contains(&r.fault_move_pct) {
            return Err("fault multipliers must be between 0 and 1000%".into());
        }
        if !self.biome_mix.is_valid() || !r.plate_mixes.iter().all(BiomeMix::is_valid) {
            return Err(
                "biome mixes need 1–99% mountains and positive desert, steppe, forest and \
                 swamp shares"
                    .into(),
            );
        }
        if self.revival.season_end_count == 0 {
            return Err("season_end_count must be positive".into());
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
        if let Some(t) = &self.traits8 {
            for size in 0..=i32::from(TRAIT_MAX) {
                let cap =
                    self.energy_max * (t.size_energy_base_pct + t.size_energy_pct * size) / 100;
                let threshold = self.repro_base + size * t.size_repro;
                if threshold > cap {
                    return Err(format!("traits8: at size {size} breeding needs more energy than the organism holds"));
                }
            }
        }
        if let Some(p) = &self.patrons {
            if p.area_radius == 0 || p.area_radius > 8 || p.target_radius > 8 {
                return Err("patrons: radii must be 1..=8".into());
            }
            if p.sickness_ppm > 1_000_000
                || p.gift_loss_ppm > 1_000_000
                || p.weather_ppm > 1_000_000
            {
                return Err("patrons: chances are in ppm, at most 1,000,000".into());
            }
            if p.max_gifts == 0 || p.max_gifts > 6 {
                return Err("patrons: max_gifts must be 1..=6".into());
            }
            if p.weather_regions_x == 0
                || p.weather_regions_y == 0
                || u32::from(p.weather_regions_x) * u32::from(p.weather_regions_y) > 64
            {
                return Err("patrons: 1 to 64 weather regions".into());
            }
            if !(0..100).contains(&p.forage_metabolism_pct) || p.blight_metabolism_pct < 0 {
                return Err("patrons: forage must be 0..100 percent, blight at least 0".into());
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

    #[test]
    fn a_year_is_not_a_multiple_of_a_day() {
        let rules = Ruleset::default();
        assert_ne!(rules.climate.epochs_per_year % rules.epochs_per_day, 0);
    }
}
