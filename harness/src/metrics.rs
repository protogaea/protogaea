//! Statistics per epoch and the ecosystem health metrics of spec §28 (the stage A1 subset).

use std::collections::{HashMap, HashSet};

use protogaea_core::genome::{DEFENSE, HUNTING, TRAIT_COUNT};
use protogaea_core::{EpochReport, Ruleset, World};
use serde::Serialize;

/// A share of the population above which a clade counts as dominant (spec §28).
const DOMINANCE_PERMILLE: u32 = 600;

#[derive(Clone, Debug, Serialize)]
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
    pub wildfires: u32,
    pub droughts: u32,
    pub plagues: u32,
    pub ticks_at_cap: u32,
    /// The mean of each trait, ×10.
    pub trait_means_x10: [u32; TRAIT_COUNT],
}

pub fn epoch_stats(world: &World, report: &EpochReport) -> EpochStats {
    let population = world.organisms.len() as u32;
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
            + report.deaths_plague,
        kills: report.deaths_predation,
        plague_deaths: report.deaths_plague,
        wildfires: report.wildfires,
        droughts: report.droughts,
        plagues: report.plagues,
        ticks_at_cap: report.ticks_at_cap,
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

/// Follows a run epoch by epoch.
pub struct Tracker {
    generation: HashMap<u64, u32>,
    pub series: Vec<EpochStats>,
    ticks: u64,
    ticks_at_cap: u64,
}

impl Tracker {
    pub fn new(world: &World) -> Self {
        Self {
            generation: world.organisms.iter().map(|o| (o.id, 0)).collect(),
            series: Vec::new(),
            ticks: 0,
            ticks_at_cap: 0,
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
        self.series.push(epoch_stats(world, report));
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

        Summary {
            seed,
            days,
            extinct: last.is_some_and(|s| s.population == 0),
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
            generations_per_day: if days > 0.0 {
                self.mean_generation() / days
            } else {
                0.0
            },
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Summary {
    pub seed: u64,
    pub days: f64,
    pub extinct: bool,
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
        ]
    }
}
