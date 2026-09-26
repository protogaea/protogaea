//! Story detectors (spec §7): they watch a running world epoch by epoch and pick out the moments
//! worth telling, each with a protagonist, what happened and how much it matters.
//!
//! The same detectors run in the world server, which puts the stories in the feed, and in the
//! balance harness, which scores every season for its stories as well as its health (roadmap,
//! stage B4). They read the world and never change it: stories are not part of consensus.

use std::collections::{BTreeMap, BTreeSet};

use protogaea_core::genome::{DEFENSE, HUNTING};
use protogaea_core::{EpochReport, Ruleset, World};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A clade must once have had this many to count as "was big" for a comeback.
const COMEBACK_WAS: u32 = 20;
/// ... then fall this low ...
const COMEBACK_LOW: u32 = 3;
/// ... and grow back to this many.
const COMEBACK_BACK: u32 = 50;
/// A crossing: this many members on another continent for this many epochs in a row.
const CROSSING_MEMBERS: u32 = 10;
const CROSSING_EPOCHS: u32 = 12;
/// "Last of its kind" and "the fall of a great clade" are for clades that reached this peak.
const GREAT_PEAK: u32 = 100;
/// Arms race: both mean hunting of hunters and mean defense of their prey grew by this much
/// (tenths of a trait point) over a world day, about 30 generations.
const ARMS_RACE_X10: i64 = 20;
/// A clade split by a closing land bridge has at least this many on each side.
const SPLIT_MEMBERS: u32 = 5;
/// Dominance is compared once per world hour, so that two clades trading the lead back and
/// forth make no story.
const HOUR: u64 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A clade that was big fell to a handful and grew back.
    Comeback,
    /// A clade established itself on another continent once the continents were apart.
    Crossing,
    /// A newcomer became the most numerous clade of a continent.
    Invasion,
    /// Hunters hunt harder and their prey defends better, together.
    ArmsRace,
    /// A single organism remains of a great clade.
    LastOfItsKind,
    /// The most numerous clade of the world changed.
    ChangingOfTheGuard,
    /// A land bridge closed with one clade living on both sides.
    SplitBySea,
    /// A great clade went extinct.
    Fall,
}

impl Kind {
    pub const ALL: [Kind; 8] = [
        Kind::Comeback,
        Kind::Crossing,
        Kind::Invasion,
        Kind::ArmsRace,
        Kind::LastOfItsKind,
        Kind::ChangingOfTheGuard,
        Kind::SplitBySea,
        Kind::Fall,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Comeback => "comeback",
            Kind::Crossing => "crossing",
            Kind::Invasion => "invasion",
            Kind::ArmsRace => "arms_race",
            Kind::LastOfItsKind => "last_of_its_kind",
            Kind::ChangingOfTheGuard => "changing_of_the_guard",
            Kind::SplitBySea => "split_by_sea",
            Kind::Fall => "fall",
        }
    }

    /// How much a story of this kind matters, before its size is taken into account.
    fn weight(self) -> u32 {
        match self {
            Kind::Comeback => 90,
            Kind::Crossing => 70,
            Kind::Invasion => 80,
            Kind::ArmsRace => 60,
            Kind::LastOfItsKind => 85,
            Kind::ChangingOfTheGuard => 50,
            Kind::SplitBySea => 75,
            Kind::Fall => 70,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Story {
    pub kind: Kind,
    pub epoch: u64,
    /// The protagonist.
    pub clade: Option<u32>,
    /// The clade it replaced, split from, and so on.
    pub other: Option<u32>,
    /// The continent (plate) where it happened, if one.
    pub plate: Option<u8>,
    /// How much it matters: the kind's weight scaled by the size of what is at stake.
    pub score: u32,
    pub data: Value,
}

impl Story {
    fn new(kind: Kind, epoch: u64, clade: Option<u32>, size: u32, data: Value) -> Self {
        // The weight, plus up to about half again for large clades.
        let bonus = (f64::from(size.max(1)).log2() * 6.0) as u32;
        Self {
            kind,
            epoch,
            clade,
            other: None,
            plate: None,
            score: kind.weight() + bonus,
            data,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct CladeTrack {
    home: Option<u8>,
    was_big: bool,
    fell: bool,
    comeback_told: bool,
    last_told: bool,
    /// Epochs in a row with enough members on each other continent.
    abroad: BTreeMap<u8, u32>,
    crossed: BTreeSet<u8>,
}

/// The detectors' memory between epochs. It serializes, so the server can keep it across
/// restarts; a fresh one simply starts noticing from the current epoch.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Detectors {
    clades: BTreeMap<u32, CladeTrack>,
    world_leader: Option<u32>,
    plate_leaders: BTreeMap<u8, u32>,
    /// Mean hunting of hunters and mean defense of the rest, ×10, a world day ago.
    arms_then: Option<(i64, i64)>,
}

/// What the detectors need to know about the world beyond its state: the plate of every cell.
pub struct Context<'a> {
    pub rules: &'a Ruleset,
    pub plates: &'a [u8],
}

impl Detectors {
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks at the world after an epoch and returns the stories it tells.
    pub fn observe(&mut self, world: &World, report: &EpochReport, ctx: &Context) -> Vec<Story> {
        let epoch = world.epoch;
        let per_day = u64::from(ctx.rules.epochs_per_day);
        // Continents only mean something once the sea starts to part them.
        let apart = epoch >= u64::from(ctx.rules.rifts.shallows_from_day) * per_day;
        let mut out = Vec::new();

        // Members of each clade on each plate.
        let mut on_plate: BTreeMap<u32, BTreeMap<u8, u32>> = BTreeMap::new();
        for o in &world.organisms {
            let plate = ctx.plates.get(usize::from(o.cell)).copied().unwrap_or(0);
            *on_plate
                .entry(o.clade_id)
                .or_default()
                .entry(plate)
                .or_default() += 1;
        }

        for (&id, clade) in &world.clades {
            let track = self.clades.entry(id).or_default();
            let plates = on_plate.get(&id);
            if track.home.is_none() {
                track.home = plates.and_then(|p| {
                    p.iter()
                        .max_by_key(|&(plate, n)| (*n, std::cmp::Reverse(*plate)))
                        .map(|(&p, _)| p)
                });
            }
            // Comeback.
            if clade.living >= COMEBACK_WAS {
                track.was_big = true;
            }
            if track.was_big && clade.living <= COMEBACK_LOW {
                track.fell = true;
            }
            if track.fell && !track.comeback_told && clade.living >= COMEBACK_BACK {
                track.comeback_told = true;
                out.push(Story::new(
                    Kind::Comeback,
                    epoch,
                    Some(id),
                    clade.living,
                    json!({ "living": clade.living, "low": COMEBACK_LOW }),
                ));
            }
            // Last of its kind.
            if clade.peak_living >= GREAT_PEAK && clade.living == 1 && !track.last_told {
                track.last_told = true;
                let last = world
                    .organisms
                    .iter()
                    .find(|o| o.clade_id == id)
                    .map(|o| o.id);
                let mut story = Story::new(
                    Kind::LastOfItsKind,
                    epoch,
                    Some(id),
                    clade.peak_living,
                    json!({ "peak": clade.peak_living, "organism": last }),
                );
                story.plate = last
                    .and_then(|oid| world.organisms.iter().find(|o| o.id == oid))
                    .map(|o| ctx.plates[usize::from(o.cell)]);
                out.push(story);
            }
            if clade.living > 1 {
                track.last_told = false;
            }
            // Crossing to another continent.
            if apart {
                if let (Some(home), Some(plates)) = (track.home, plates) {
                    for (&plate, &n) in plates {
                        if plate == home || track.crossed.contains(&plate) {
                            continue;
                        }
                        let run = track.abroad.entry(plate).or_default();
                        *run = if n >= CROSSING_MEMBERS { *run + 1 } else { 0 };
                        if *run >= CROSSING_EPOCHS {
                            track.crossed.insert(plate);
                            let mut story = Story::new(
                                Kind::Crossing,
                                epoch,
                                Some(id),
                                n,
                                json!({ "from": home, "to": plate, "living": n }),
                            );
                            story.plate = Some(plate);
                            out.push(story);
                        }
                    }
                }
            }
        }

        // The fall of a great clade.
        for c in &report.clades_extinct {
            if c.peak_living >= GREAT_PEAK {
                out.push(Story::new(
                    Kind::Fall,
                    epoch,
                    Some(c.id),
                    c.peak_living,
                    json!({ "peak": c.peak_living, "founded": c.founded_epoch }),
                ));
            }
            self.clades.remove(&c.id);
        }

        // A land bridge closed: clades living on both sides are split by the sea.
        if !report.bridges_closed.is_empty() {
            for (&id, plates) in &on_plate {
                let sides: Vec<(u8, u32)> = plates
                    .iter()
                    .filter(|&(_, &n)| n >= SPLIT_MEMBERS)
                    .map(|(&p, &n)| (p, n))
                    .collect();
                if sides.len() >= 2 {
                    let size: u32 = sides.iter().map(|s| s.1).sum();
                    out.push(Story::new(
                        Kind::SplitBySea,
                        epoch,
                        Some(id),
                        size,
                        json!({ "bridges": report.bridges_closed, "sides": sides }),
                    ));
                }
            }
        }

        if epoch.is_multiple_of(HOUR) {
            // Changing of the guard: the world's most numerous clade.
            let leader = world
                .clades
                .values()
                .max_by_key(|c| (c.living, std::cmp::Reverse(c.id)))
                .map(|c| (c.id, c.living));
            if let Some((id, living)) = leader {
                if let Some(old) = self.world_leader {
                    if old != id {
                        let mut story = Story::new(
                            Kind::ChangingOfTheGuard,
                            epoch,
                            Some(id),
                            living,
                            json!({ "living": living, "population": world.organisms.len() }),
                        );
                        story.other = Some(old);
                        out.push(story);
                    }
                }
                self.world_leader = Some(id);
            }
            // Invasion: a newcomer leads a continent.
            if apart {
                let mut by_plate: BTreeMap<u8, (u32, u32)> = BTreeMap::new();
                for (&id, plates) in &on_plate {
                    for (&plate, &n) in plates {
                        let best = by_plate.entry(plate).or_insert((id, 0));
                        if n > best.1 || (n == best.1 && id < best.0) {
                            *best = (id, n);
                        }
                    }
                }
                for (plate, (id, n)) in by_plate {
                    let old = self.plate_leaders.insert(plate, id);
                    let newcomer = self
                        .clades
                        .get(&id)
                        .and_then(|t| t.home)
                        .is_some_and(|home| home != plate);
                    if let Some(old) = old {
                        if old != id && newcomer && n >= CROSSING_MEMBERS {
                            let mut story = Story::new(
                                Kind::Invasion,
                                epoch,
                                Some(id),
                                n,
                                json!({ "living": n }),
                            );
                            story.other = Some(old);
                            story.plate = Some(plate);
                            out.push(story);
                        }
                    }
                }
            }
        }

        // Arms race, once per world day.
        if epoch.is_multiple_of(per_day) && epoch > 0 {
            let (mut hunt, mut hunters, mut def, mut prey) = (0i64, 0i64, 0i64, 0i64);
            for o in &world.organisms {
                if o.genome.traits[HUNTING] >= 4 {
                    hunt += i64::from(o.genome.traits[HUNTING]);
                    hunters += 1;
                } else {
                    def += i64::from(o.genome.traits[DEFENSE]);
                    prey += 1;
                }
            }
            if hunters > 0 && prey > 0 {
                let now = (hunt * 10 / hunters, def * 10 / prey);
                if let Some(then) = self.arms_then {
                    if now.0 - then.0 >= ARMS_RACE_X10 && now.1 - then.1 >= ARMS_RACE_X10 {
                        out.push(Story::new(Kind::ArmsRace, epoch, None, hunters as u32, json!({ "hunting_x10": [then.0, now.0], "defense_x10": [then.1, now.1] })));
                    }
                }
                self.arms_then = Some(now);
            }
        }
        out
    }
}

/// The stories of one world day worth showing: the best of each kind first, at most `limit`.
pub fn best_of(stories: &[Story], limit: usize) -> Vec<&Story> {
    let mut sorted: Vec<&Story> = stories.iter().collect();
    sorted.sort_by(|a, b| b.score.cmp(&a.score).then(a.epoch.cmp(&b.epoch)));
    let mut seen = BTreeSet::new();
    let (mut first, mut rest): (Vec<&Story>, Vec<&Story>) = (Vec::new(), Vec::new());
    for s in sorted {
        if seen.insert(s.kind) {
            first.push(s);
        } else {
            rest.push(s);
        }
    }
    first.into_iter().chain(rest).take(limit).collect()
}

#[cfg(test)]
mod tests {
    use protogaea_core::run::Run;

    use super::*;

    /// Over a short season the detectors find something, and never the same crossing twice.
    #[test]
    fn a_compressed_season_tells_stories() {
        let mut rules = Ruleset::default();
        rules.rifts.fault_day = 0;
        rules.rifts.shallows_from_day = 0;
        rules.rifts.deep_from_day = 1;
        rules.rifts.bridges_from_day = 1;
        rules.rifts.bridges_to_day = 2;
        let mut run = Run::new(3, rules);
        let mut detectors = Detectors::new();
        let mut stories = Vec::new();
        for _ in 0..(3 * run.rules.epochs_per_day) {
            let report = run.step();
            let ctx = Context {
                rules: &run.rules,
                plates: &run.plan.plates,
            };
            stories.extend(detectors.observe(&run.world, &report, &ctx));
        }
        assert!(!stories.is_empty());
        let crossings: Vec<_> = stories
            .iter()
            .filter(|s| s.kind == Kind::Crossing)
            .map(|s| (s.clade, s.plate))
            .collect();
        let unique: BTreeSet<_> = crossings.iter().collect();
        assert_eq!(crossings.len(), unique.len());
    }
}
