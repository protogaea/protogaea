//! Statistics per epoch and the ecosystem health metrics of spec §28 (the subset measured so
//! far).

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use protogaea_core::genome::{DEFENSE, HUNTING, TRAIT_COUNT};
use protogaea_core::rifts::Plan;
use protogaea_core::{EpochReport, Genome, Ruleset, World};
use serde::{Deserialize, Serialize};

/// A share of the population above which a clade counts as dominant (spec §28).
const DOMINANCE_PERMILLE: u32 = 600;

/// A group of passable cells counts as a continent if it has at least this many land cells.
const CONTINENT_MIN_LAND: u32 = 40;

/// Divergence after the breakup (spec §28): by the end of the season the continents' clade
/// makeup differs by at least this much, and their mean hues by at least this angle.
const FAUNA_APART_PERMILLE: u32 = 800;
const HUES_APART_DEGREES: f64 = 30.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpochStats {
    pub epoch: u64,
    pub population: u32,
    /// Hunting ≥ 4.
    pub hunters: u32,
    /// Defense ≥ 6, not a hunter.
    pub armored: u32,
    /// Everyone else.
    pub grazers: u32,
    pub clades: u32,
    pub clades_20: u32,
    pub dominant_clade: u32,
    /// The most numerous clade's share of the population, in permille.
    pub dominant_permille: u32,
    pub births: u32,
    pub deaths: u32,
    pub kills: u32,
    pub plague_deaths: u32,
    pub drowned: u32,
    pub floods: u32,
    pub wildfires: u32,
    pub droughts: u32,
    pub plagues: u32,
    /// 1 if the spore bank revived the world in this epoch.
    pub revivals: u32,
    pub ticks_at_cap: u32,
    /// Landmasses that organisms cannot walk between.
    pub continents: u32,
    /// The mean genome distance between the dominant clades of different plates, ×10.
    pub divergence_x10: u32,
    /// The mean distance between the mean genomes of different plates, ×10: a smooth companion
    /// to `divergence_x10`, which moves only when clades split.
    #[serde(default)]
    pub centroid_divergence_x10: u32,
    /// How different the plates' clade makeup is, in permille (`composition_permille`).
    #[serde(default)]
    pub composition_permille: u32,
    /// The mean angle between the plates' mean hues, ×10 degrees.
    #[serde(default)]
    pub hue_divergence_x10: u32,
    /// The mean of each trait, ×10.
    pub trait_means_x10: [u32; TRAIT_COUNT],
}

pub fn epoch_stats(
    world: &World,
    report: &EpochReport,
    plates: &[u8],
    continents: u32,
) -> EpochStats {
    let population = world.organisms.len() as u32;
    let profiles = plate_profiles(world, plates);
    let mut stats = EpochStats {
        epoch: world.epoch,
        population,
        hunters: 0,
        armored: 0,
        grazers: 0,
        clades: world.clades.len() as u32,
        clades_20: world.clades.values().filter(|c| c.living >= 20).count() as u32,
        dominant_clade: 0,
        dominant_permille: 0,
        births: report.births,
        deaths: report.deaths_starvation
            + report.deaths_old_age
            + report.deaths_predation
            + report.deaths_plague
            + report.deaths_drowned,
        kills: report.deaths_predation,
        plague_deaths: report.deaths_plague,
        drowned: report.deaths_drowned,
        floods: report.floods,
        wildfires: report.wildfires,
        droughts: report.droughts,
        plagues: report.plagues,
        revivals: u32::from(report.revival),
        ticks_at_cap: report.ticks_at_cap,
        continents,
        divergence_x10: divergence_x10(world, plates),
        centroid_divergence_x10: centroid_divergence_x10(&profiles),
        composition_permille: composition_permille(&profiles),
        hue_divergence_x10: (hue_divergence(&profiles) * 10.0).round() as u32,
        trait_means_x10: [0; TRAIT_COUNT],
    };
    let mut sums = [0u64; TRAIT_COUNT];
    for o in &world.organisms {
        let t = o.genome.traits;
        if t[HUNTING] >= 4 {
            stats.hunters += 1;
        } else if t[DEFENSE] >= 6 {
            stats.armored += 1;
        } else {
            stats.grazers += 1;
        }
        for (sum, &value) in sums.iter_mut().zip(t.iter()) {
            *sum += u64::from(value);
        }
    }
    if population > 0 {
        for (mean, sum) in stats.trait_means_x10.iter_mut().zip(sums) {
            *mean = (sum * 10 / u64::from(population)) as u32;
        }
        if let Some(c) = world
            .clades
            .values()
            .max_by_key(|c| (c.living, std::cmp::Reverse(c.id)))
        {
            stats.dominant_clade = c.id;
            stats.dominant_permille = (u64::from(c.living) * 1000 / u64::from(population)) as u32;
        }
    }
    stats
}

/// The number of groups of passable cells (8-neighborhood) with at least `CONTINENT_MIN_LAND`
/// land cells.
pub fn continents(world: &World, rules: &Ruleset) -> u32 {
    let passable = |i: usize| rules.biomes[world.cells[i].biome as usize].passable;
    let mut seen = vec![false; world.cells.len()];
    let mut count = 0;
    for start in 0..world.cells.len() {
        if seen[start] || !passable(start) {
            continue;
        }
        seen[start] = true;
        let mut queue = VecDeque::from([start]);
        let mut land = 0u32;
        while let Some(i) = queue.pop_front() {
            land += u32::from(world.cells[i].biome.is_land());
            let (x, y) = world.coords(i);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if let Some(j) = world.index(x + dx, y + dy) {
                        if !seen[j] && passable(j) {
                            seen[j] = true;
                            queue.push_back(j);
                        }
                    }
                }
            }
        }
        if land >= CONTINENT_MIN_LAND {
            count += 1;
        }
    }
    count
}

/// The mean genome distance, ×10, between the dominant clades of different plates (the future
/// continents), over the pairs of plates that have organisms.
pub fn divergence_x10(world: &World, plates: &[u8]) -> u32 {
    let mut counts: BTreeMap<(u8, u32), u32> = BTreeMap::new();
    for o in &world.organisms {
        *counts
            .entry((plates[usize::from(o.cell)], o.clade_id))
            .or_insert(0) += 1;
    }
    // For each plate, the clade with the most organisms there; ties go to the lower id.
    let mut dominant: BTreeMap<u8, (u32, u32)> = BTreeMap::new();
    for (&(plate, clade), &n) in &counts {
        let best = dominant.entry(plate).or_insert((0, clade));
        if n > best.0 {
            *best = (n, clade);
        }
    }
    let references: Vec<Genome> = dominant
        .values()
        .filter_map(|&(_, clade)| world.clades.get(&clade).map(|c| c.reference))
        .collect();
    let mut sum = 0u64;
    let mut pairs = 0u64;
    for (i, a) in references.iter().enumerate() {
        for b in &references[i + 1..] {
            sum += u64::from(a.distance(b));
            pairs += 1;
        }
    }
    (sum * 10).checked_div(pairs).unwrap_or(0) as u32
}

/// The organisms of one plate (a future continent).
#[derive(Clone, Debug)]
pub struct PlateProfile {
    pub plate: u8,
    pub population: u32,
    pub dominant_clade: u32,
    /// The dominant clade's share of the plate's population, in permille.
    pub dominant_permille: u32,
    /// The mean of each trait on the plate, ×10.
    pub trait_means_x10: [u32; TRAIT_COUNT],
    /// Organisms per clade on the plate.
    pub clades: BTreeMap<u32, u32>,
    /// The circular mean of the neutral `hue` gene, in degrees.
    pub hue_mean: f64,
}

/// Counts gathered for one plate.
#[derive(Default)]
struct PlateTally {
    population: u32,
    trait_sums: [u64; TRAIT_COUNT],
    clades: BTreeMap<u32, u32>,
    hue_x: f64,
    hue_y: f64,
}

/// Profiles of the plates that have organisms, in plate order.
pub fn plate_profiles(world: &World, plates: &[u8]) -> Vec<PlateProfile> {
    let mut by_plate: BTreeMap<u8, PlateTally> = BTreeMap::new();
    for o in &world.organisms {
        let tally = by_plate.entry(plates[usize::from(o.cell)]).or_default();
        tally.population += 1;
        for (sum, &value) in tally.trait_sums.iter_mut().zip(o.genome.traits.iter()) {
            *sum += u64::from(value);
        }
        *tally.clades.entry(o.clade_id).or_insert(0) += 1;
        let angle = f64::from(o.genome.hue).to_radians();
        tally.hue_x += angle.cos();
        tally.hue_y += angle.sin();
    }
    by_plate
        .into_iter()
        .map(|(plate, tally)| {
            // Ties go to the lower clade id.
            let (dominant_clade, top) =
                tally.clades.iter().fold(
                    (0, 0),
                    |best, (&c, &n)| if n > best.1 { (c, n) } else { best },
                );
            let population = tally.population;
            PlateProfile {
                plate,
                population,
                dominant_clade,
                dominant_permille: top * 1000 / population,
                trait_means_x10: tally
                    .trait_sums
                    .map(|s| (s * 10 / u64::from(population)) as u32),
                hue_mean: tally
                    .hue_y
                    .atan2(tally.hue_x)
                    .to_degrees()
                    .rem_euclid(360.0),
                clades: tally.clades,
            }
        })
        .collect()
}

/// How different the clade makeup of the plates is, in permille, averaged over pairs of
/// plates: the Bray–Curtis dissimilarity of their clade shares. 0 means the same clades in
/// the same proportions; 1000 means no clade in common.
pub fn composition_permille(profiles: &[PlateProfile]) -> u32 {
    let mut sum = 0.0;
    let mut pairs = 0u32;
    for (i, a) in profiles.iter().enumerate() {
        for b in &profiles[i + 1..] {
            let shared: f64 = a
                .clades
                .iter()
                .filter_map(|(clade, &na)| {
                    b.clades.get(clade).map(|&nb| {
                        (f64::from(na) / f64::from(a.population))
                            .min(f64::from(nb) / f64::from(b.population))
                    })
                })
                .sum();
            sum += 1.0 - shared;
            pairs += 1;
        }
    }
    if pairs == 0 {
        0
    } else {
        (sum * 1000.0 / f64::from(pairs)).round() as u32
    }
}

/// The mean angle, in degrees (0–180), between the plates' mean hues, over pairs of plates.
pub fn hue_divergence(profiles: &[PlateProfile]) -> f64 {
    let mut sum = 0.0;
    let mut pairs = 0u32;
    for (i, a) in profiles.iter().enumerate() {
        for b in &profiles[i + 1..] {
            let d = (a.hue_mean - b.hue_mean).rem_euclid(360.0);
            sum += d.min(360.0 - d);
            pairs += 1;
        }
    }
    if pairs == 0 {
        0.0
    } else {
        sum / f64::from(pairs)
    }
}

/// The mean distance, ×10, between the mean genomes of different plates, in mutation steps
/// (half the sum of the absolute trait differences, as in `Genome::distance`).
pub fn centroid_divergence_x10(profiles: &[PlateProfile]) -> u32 {
    let mut sum = 0u64;
    let mut pairs = 0u64;
    for (i, a) in profiles.iter().enumerate() {
        for b in &profiles[i + 1..] {
            let d: u32 = a
                .trait_means_x10
                .iter()
                .zip(b.trait_means_x10.iter())
                .map(|(&x, &y)| x.abs_diff(y))
                .sum();
            sum += u64::from(d / 2);
            pairs += 1;
        }
    }
    sum.checked_div(pairs).unwrap_or(0) as u32
}

/// Follows a run epoch by epoch.
#[derive(Serialize, Deserialize)]
pub struct Tracker {
    generation: HashMap<u64, u32>,
    pub series: Vec<EpochStats>,
    ticks: u64,
    ticks_at_cap: u64,
    /// The plate of each cell, from the rift plan.
    plates: Vec<u8>,
    plate_count: u8,
    continents: u32,
    /// The divergence when the rifts began to turn into shallows (spec §4, phase III).
    divergence_start_x10: Option<u32>,
    #[serde(default)]
    centroid_start_x10: Option<u32>,
    #[serde(default)]
    composition_start_permille: Option<u32>,
    revivals: u32,
    drowned: u32,
    ended: bool,
}

impl Tracker {
    pub fn new(world: &World, rules: &Ruleset, plan: &Plan) -> Self {
        Self {
            generation: world.organisms.iter().map(|o| (o.id, 0)).collect(),
            series: Vec::new(),
            ticks: 0,
            ticks_at_cap: 0,
            plates: plan.plates.clone(),
            plate_count: plan.plate_count,
            continents: continents(world, rules),
            divergence_start_x10: None,
            centroid_start_x10: None,
            composition_start_permille: None,
            revivals: 0,
            drowned: 0,
            ended: world.ended,
        }
    }

    pub fn record(&mut self, world: &World, report: &EpochReport, rules: &Ruleset) {
        for &(parent, child) in &report.births_list {
            let g = self.generation.get(&parent).copied().unwrap_or(0) + 1;
            self.generation.insert(child, g);
        }
        let living: HashSet<u64> = world.organisms.iter().map(|o| o.id).collect();
        self.generation.retain(|id, _| living.contains(id));
        self.ticks += u64::from(rules.ticks_per_epoch);
        self.ticks_at_cap += u64::from(report.ticks_at_cap);
        if report.rift_changes > 0 {
            self.continents = continents(world, rules);
        }
        let stats = epoch_stats(world, report, &self.plates, self.continents);
        let phase_3 = u64::from(rules.rifts.shallows_from_day) * u64::from(rules.epochs_per_day);
        if self.divergence_start_x10.is_none() && world.epoch >= phase_3 {
            self.divergence_start_x10 = Some(stats.divergence_x10);
            self.centroid_start_x10 = Some(stats.centroid_divergence_x10);
            self.composition_start_permille = Some(stats.composition_permille);
        }
        self.revivals += stats.revivals;
        self.drowned += stats.drowned;
        self.ended = world.ended;
        self.series.push(stats);
    }

    /// Keeps only the most recent `max` epochs of the series (live mode).
    pub fn trim(&mut self, max: usize) {
        if self.series.len() > max {
            let excess = self.series.len() - max;
            self.series.drain(..excess);
        }
    }

    pub fn mean_generation(&self) -> f64 {
        if self.generation.is_empty() {
            return 0.0;
        }
        self.generation.values().map(|&g| f64::from(g)).sum::<f64>() / self.generation.len() as f64
    }

    pub fn summary(&self, seed: u64, rules: &Ruleset) -> Summary {
        let per_day = u64::from(rules.epochs_per_day);
        let epochs = self.series.len() as u64;
        let days = epochs as f64 / per_day as f64;
        let last = self.series.last();
        // Generations are counted from genesis, even when the series keeps only recent epochs.
        let days_since_genesis = last.map_or(0.0, |s| s.epoch as f64 / per_day as f64);
        let last_epoch = last.map_or(0, |s| s.epoch);

        let last_day = &self.series[self.series.len().saturating_sub(per_day as usize)..];
        let mean_last_day = if last_day.is_empty() {
            0.0
        } else {
            last_day
                .iter()
                .map(|s| f64::from(s.population))
                .sum::<f64>()
                / last_day.len() as f64
        };

        let after_day_3: Vec<u32> = self
            .series
            .iter()
            .filter(|s| s.epoch > 3 * per_day)
            .map(|s| s.clades_20)
            .collect();

        // Changes of the dominant clade, sampled hourly to ignore flicker.
        let hourly = (per_day / 24).max(1) as usize;
        let samples: Vec<u32> = self
            .series
            .iter()
            .step_by(hourly)
            .filter(|s| s.population > 0)
            .map(|s| s.dominant_clade)
            .collect();
        let changes = samples.windows(2).filter(|w| w[0] != w[1]).count();

        let mut longest = 0u64;
        let mut streak = 0u64;
        for s in &self.series {
            if s.dominant_permille > DOMINANCE_PERMILLE {
                streak += 1;
                longest = longest.max(streak);
            } else {
                streak = 0;
            }
        }

        let has_rifts = self.plate_count > 1;
        let reached_breakup =
            has_rifts && last_epoch >= u64::from(rules.rifts.bridges_to_day) * per_day;
        let reached_season_end = last_epoch >= u64::from(rules.season_days) * per_day;
        let divergence_growth = match (self.divergence_start_x10, last) {
            (Some(start), Some(end)) if has_rifts && reached_season_end => {
                Some((f64::from(end.divergence_x10) - f64::from(start)) / 10.0)
            }
            _ => None,
        };
        let centroid_divergence_growth = match (self.centroid_start_x10, last) {
            (Some(start), Some(end)) if has_rifts && reached_season_end => {
                Some((f64::from(end.centroid_divergence_x10) - f64::from(start)) / 10.0)
            }
            _ => None,
        };

        Summary {
            seed,
            days,
            extinct: self.series.iter().any(|s| s.population == 0),
            ended_by_extinction: self.ended,
            final_population: last.map_or(0, |s| s.population),
            min_population: self.series.iter().map(|s| s.population).min().unwrap_or(0),
            final_hunters: last.map_or(0, |s| s.hunters),
            final_clades_20: last.map_or(0, |s| s.clades_20),
            equilibrium_pct: mean_last_day * 100.0 / f64::from(rules.max_organisms),
            min_clades_20_after_day_3: after_day_3.iter().copied().min(),
            dominant_changes_per_3_days: if days > 0.0 {
                changes as f64 / days * 3.0
            } else {
                0.0
            },
            longest_dominance_days: longest as f64 / per_day as f64,
            cap_ticks_pct: if self.ticks > 0 {
                self.ticks_at_cap as f64 * 100.0 / self.ticks as f64
            } else {
                0.0
            },
            generations_per_day: if days_since_genesis > 0.0 {
                self.mean_generation() / days_since_genesis
            } else {
                0.0
            },
            revivals: self.revivals,
            drowned: self.drowned,
            plates: self.plate_count,
            final_continents: self.continents,
            reached_breakup,
            divergence_growth,
            centroid_divergence_growth,
            start_composition_permille: self.composition_start_permille,
            divergence_judged: has_rifts && reached_season_end,
            final_composition_permille: last.map_or(0, |s| s.composition_permille),
            final_hue_divergence: last.map_or(0.0, |s| f64::from(s.hue_divergence_x10) / 10.0),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Summary {
    pub seed: u64,
    pub days: f64,
    /// The population fell to zero at some point.
    pub extinct: bool,
    /// The spore bank had to revive the world too often, and the season ended (spec §12).
    pub ended_by_extinction: bool,
    pub final_population: u32,
    pub min_population: u32,
    pub final_hunters: u32,
    pub final_clades_20: u32,
    /// Mean population over the last world day, as a share of `max_organisms`.
    pub equilibrium_pct: f64,
    pub min_clades_20_after_day_3: Option<u32>,
    pub dominant_changes_per_3_days: f64,
    /// The longest stretch with one clade above 60% of the population.
    pub longest_dominance_days: f64,
    pub cap_ticks_pct: f64,
    pub generations_per_day: f64,
    /// Natural revivals from the spore bank.
    pub revivals: u32,
    pub drowned: u32,
    /// Plates in the rift plan: the continents the world should end with.
    pub plates: u8,
    pub final_continents: u32,
    /// The run lasted until the last land bridge closed.
    pub reached_breakup: bool,
    /// How much the divergence between the continents' dominant clades grew from the start of
    /// phase III to the end of the season, in mutation steps (spec §28).
    pub divergence_growth: Option<f64>,
    /// The same growth for the distance between the plates' mean genomes; reported only.
    pub centroid_divergence_growth: Option<f64>,
    /// The run has rifts and reached the end of the season, so divergence can be judged.
    pub divergence_judged: bool,
    /// At the end: how different the plates' clade makeup is, in permille, and the mean
    /// angle between their mean hues, in degrees (spec §28).
    pub final_composition_permille: u32,
    /// The clade makeup difference when the rifts began to turn into shallows.
    pub start_composition_permille: Option<u32>,
    pub final_hue_divergence: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub name: &'static str,
    /// `None` when the run is too short to judge.
    pub pass: Option<bool>,
}

impl Summary {
    /// Pass conditions from spec §28 measured so far. The thresholds are candidates.
    pub fn checks(&self) -> Vec<Check> {
        let check = |name, pass| Check { name, pass };
        vec![
            check("no total extinction", Some(!self.extinct)),
            check("predators survive to the end", Some(self.final_hunters > 0)),
            check(
                "the spore bank revives the world at most once",
                Some(self.revivals <= 1),
            ),
            check(
                "at least 6 clades of 20+ after day 3",
                self.min_clades_20_after_day_3.map(|m| m >= 6),
            ),
            check(
                "dominant clade changes at least once per 3 days",
                Some(self.dominant_changes_per_3_days >= 1.0),
            ),
            check(
                "no clade above 60% for more than 3 days",
                Some(self.longest_dominance_days <= 3.0),
            ),
            check(
                "under 1% of ticks at the global limit",
                Some(self.cap_ticks_pct < 1.0),
            ),
            check(
                "equilibrium at 40–70% of the limit",
                Some((40.0..=70.0).contains(&self.equilibrium_pct)),
            ),
            check(
                "at least 30 generations per world day",
                Some(self.generations_per_day >= 30.0),
            ),
            check(
                "each plate ends as its own continent",
                self.reached_breakup
                    .then_some(self.final_continents == u32::from(self.plates)),
            ),
            check(
                "continents end with their own fauna (clades 80%+ apart, hues 30°+)",
                self.divergence_judged.then_some(
                    self.final_composition_permille >= FAUNA_APART_PERMILLE
                        && self.final_hue_divergence >= HUES_APART_DEGREES,
                ),
            ),
        ]
    }
}
