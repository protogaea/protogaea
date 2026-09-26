//! What the server records about each epoch: its header and its events. None of it is part of
//! consensus; all of it can be recomputed from the world (spec §22).

use std::collections::BTreeMap;

use protogaea_core::genome::{DEFENSE, HUNTING};
use protogaea_core::run::{hex, season_phase};
use protogaea_core::{EffectKind, EpochReport, Ruleset, StateRoots, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The header of an epoch: the state it ended with and its headline numbers. Epoch `n` is the
/// state after `n` epochs have run; epoch 0 is genesis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Header {
    pub epoch: u64,
    pub day: f64,
    /// The phase of Season 1 (spec §4): its number and name.
    pub phase: u8,
    pub phase_name: String,
    pub state_root: String,
    pub roots: Roots,
    pub population: u32,
    pub grazers: u32,
    pub armored: u32,
    pub hunters: u32,
    /// Clades with living members, and those with 20 or more.
    pub clades: u32,
    pub clades_20: u32,
    pub dominant_clade: u32,
    /// The dominant clade's share of the population, in permille.
    pub dominant_permille: u32,
    pub births: u32,
    pub deaths: Deaths,
    pub wildfires: u32,
    pub floods: u32,
    pub droughts: u32,
    pub plagues: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Deaths {
    pub starvation: u32,
    pub old_age: u32,
    pub predation: u32,
    pub plague: u32,
    pub drowned: u32,
}

/// The subtree roots of `state_root`, in hex (spec §15).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Roots {
    pub globals: String,
    pub cells: String,
    pub organisms: String,
    pub clades: String,
    pub museum: String,
    pub effects: String,
    pub rifts: String,
    pub spore_bank: String,
    pub revivals: String,
}

impl From<&StateRoots> for Roots {
    fn from(r: &StateRoots) -> Self {
        Self {
            globals: hex(&r.globals),
            cells: hex(&r.cells),
            organisms: hex(&r.organisms),
            clades: hex(&r.clades),
            museum: hex(&r.museum),
            effects: hex(&r.effects),
            rifts: hex(&r.rifts),
            spore_bank: hex(&r.spore_bank),
            revivals: hex(&r.revivals),
        }
    }
}

/// The archetype an organism is counted as: 0 grazer, 1 armored, 2 hunter (as in the harness).
pub fn archetype(traits: &[u8]) -> u8 {
    if traits[HUNTING] >= 4 {
        2
    } else if traits[DEFENSE] >= 6 {
        1
    } else {
        0
    }
}

pub fn header(world: &World, report: &EpochReport, rules: &Ruleset) -> Header {
    let roots = world.state_roots();
    let mut kinds = [0u32; 3];
    for o in &world.organisms {
        kinds[usize::from(archetype(&o.genome.traits))] += 1;
    }
    let population = world.organisms.len() as u32;
    let (dominant_clade, dominant_living) = world
        .clades
        .values()
        .map(|c| (c.id, c.living))
        .max_by_key(|&(id, living)| (living, std::cmp::Reverse(id)))
        .unwrap_or((0, 0));
    let (phase, phase_name) = season_phase(rules, world.epoch);
    Header {
        epoch: world.epoch,
        day: world.epoch as f64 / f64::from(rules.epochs_per_day),
        phase,
        phase_name: phase_name.to_string(),
        state_root: hex(&roots.root()),
        roots: Roots::from(&roots),
        population,
        grazers: kinds[0],
        armored: kinds[1],
        hunters: kinds[2],
        clades: world.clades.len() as u32,
        clades_20: world.clades.values().filter(|c| c.living >= 20).count() as u32,
        dominant_clade,
        dominant_permille: (dominant_living * 1000)
            .checked_div(population)
            .unwrap_or(0),
        births: report.births,
        deaths: Deaths {
            starvation: report.deaths_starvation,
            old_age: report.deaths_old_age,
            predation: report.deaths_predation,
            plague: report.deaths_plague,
            drowned: report.deaths_drowned,
        },
        wildfires: report.wildfires,
        floods: report.floods,
        droughts: report.droughts,
        plagues: report.plagues,
    }
}

/// A structured event of the feed (spec §7). Story detectors come later (stage B5); these are the
/// facts they will be built from.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub kind: String,
    pub clade_id: Option<u32>,
    pub organism_id: Option<u64>,
    pub data: Value,
}

impl Event {
    fn new(kind: &str, clade_id: Option<u32>, data: Value) -> Self {
        Self {
            kind: kind.to_string(),
            clade_id,
            organism_id: None,
            data,
        }
    }
}

/// The dominant clade is compared once per world hour, as in the harness, so that two clades
/// trading the lead back and forth do not flood the feed.
pub const DOMINANT_EVERY: u64 = 12;

/// What the world looked like before an epoch, as far as events need to know.
pub struct Before {
    pub epoch: u64,
    pub peaks: BTreeMap<u32, u32>,
    /// The dominant clade at the last full world hour.
    pub hour_dominant: u32,
}

impl Before {
    pub fn of(world: &World, hour_dominant: u32) -> Self {
        Self {
            epoch: world.epoch,
            peaks: world
                .clades
                .iter()
                .map(|(&id, c)| (id, c.peak_living))
                .collect(),
            hour_dominant,
        }
    }
}

pub fn events(
    before: &Before,
    world: &World,
    report: &EpochReport,
    header: &Header,
    rules: &Ruleset,
) -> Vec<Event> {
    let mut out = Vec::new();
    let named = |peak: u32| peak >= rules.clade_name_threshold;

    let (phase_before, _) = season_phase(rules, before.epoch);
    if header.phase != phase_before {
        out.push(Event::new(
            "phase",
            None,
            json!({ "phase": header.phase, "name": header.phase_name }),
        ));
    }
    for &id in &report.clades_founded {
        let clade = world
            .clades
            .get(&id)
            .or_else(|| report.clades_extinct.iter().find(|c| c.id == id));
        if let Some(c) = clade {
            out.push(Event::new(
                "clade_founded",
                Some(id),
                json!({ "parent_id": c.parent_id, "hue": c.reference.hue }),
            ));
        }
    }
    for c in world.clades.values() {
        let was = before.peaks.get(&c.id).copied().unwrap_or(0);
        if named(c.peak_living) && !named(was) {
            out.push(Event::new(
                "clade_named",
                Some(c.id),
                json!({ "living": c.living }),
            ));
        }
    }
    for c in &report.clades_extinct {
        out.push(Event::new(
            "clade_extinct",
            Some(c.id),
            json!({
                "peak_living": c.peak_living,
                "founded_epoch": c.founded_epoch,
                "named": named(c.peak_living),
            }),
        ));
    }
    if header.epoch.is_multiple_of(DOMINANT_EVERY)
        && header.dominant_clade != before.hour_dominant
        && header.population > 0
    {
        out.push(Event::new(
            "dominant_changed",
            Some(header.dominant_clade),
            json!({ "from": before.hour_dominant, "permille": header.dominant_permille }),
        ));
    }
    for &bridge in &report.bridges_closed {
        out.push(Event::new(
            "bridge_closed",
            None,
            json!({ "bridge": bridge }),
        ));
    }
    // Effects are kept in the order they started, so this epoch's are the last of their kind.
    for (kind, name, count) in [
        (EffectKind::Ash, "wildfire", report.wildfires),
        (EffectKind::Drought, "drought", report.droughts),
        (EffectKind::Flood, "flood", report.floods),
    ] {
        let started: Vec<_> = world
            .effects
            .iter()
            .filter(|e| e.kind == kind)
            .rev()
            .take(count as usize)
            .collect();
        for e in started.into_iter().rev() {
            out.push(Event::new(
                name,
                None,
                json!({ "cell": e.center, "radius": e.radius }),
            ));
        }
    }
    if report.plagues > 0 {
        out.push(Event::new(
            "plague",
            None,
            json!({ "outbreaks": report.plagues, "deaths": report.deaths_plague }),
        ));
    }
    if report.revival {
        out.push(Event::new(
            "revival",
            None,
            json!({ "organisms": report.revived }),
        ));
    }
    out
}
