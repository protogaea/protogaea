//! The world loop: advances the authoritative world on a timer and records every epoch.
//!
//! Order per epoch: step the world, write the epoch to the database in one transaction, then
//! replace the latest snapshot. After a crash between the two writes, the restart drops the
//! database rows past the snapshot and recomputes them; the world is deterministic, so they come
//! out the same.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use protogaea_core::miracle::Outcomes;
use protogaea_core::run::Run;
use protogaea_core::{EpochReport, Miracle, Ruleset, World};
use serde::{Deserialize, Serialize};

use protogaea_stories::{Context, Detectors};

use crate::intake::Intake;
use crate::model::{self, Before, Header, DOMINANT_EVERY};
use crate::store::{EpochRecord, Store};

const LATEST: &str = "latest.json";
pub const DB: &str = "protogaea.db";

pub struct Options {
    pub seed: u64,
    pub rules: Ruleset,
    pub data: PathBuf,
    pub epoch_seconds: u64,
    /// Snapshots kept for the time machine, one every this many epochs.
    pub archive_every: u64,
    /// The floor of the price of a miracle, in work units.
    pub price_min: u128,
    /// Seed epochs from drand (the live season) or from the stand-in beacon (offline runs).
    pub drand: bool,
    /// Hashes a spark is worth in the first window, for load tests (the default is 256).
    pub first_weight: Option<u64>,
}

/// What the API reads while the loop runs.
pub struct Live {
    pub world: World,
    pub header: Header,
    /// When the next epoch is due, in Unix milliseconds.
    pub next_epoch_ms: u64,
    /// The dominant clade at the last full world hour (`model::DOMINANT_EVERY`).
    pub hour_dominant: u32,
}

pub struct Shared {
    pub live: RwLock<Live>,
    /// Wishes and sparks: the open window and the spark log (stage C).
    pub intake: Mutex<Intake>,
    pub seed: u64,
    pub rules: Ruleset,
    pub data: PathBuf,
    pub epoch_seconds: u64,
    pub archive_every: u64,
    pub drand: bool,
}

impl Shared {
    pub fn db(&self) -> PathBuf {
        self.data.join(DB)
    }

    /// The path of an archived snapshot, whether or not it exists.
    pub fn archived(&self, epoch: u64) -> PathBuf {
        archive_path(&self.data, epoch)
    }
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    seed: u64,
    rules: Ruleset,
    world: World,
    /// The story detectors' memory, so a restart neither loses nor repeats a story.
    #[serde(default)]
    detectors: Detectors,
}

#[derive(Serialize)]
struct SnapshotRef<'a> {
    seed: u64,
    rules: &'a Ruleset,
    world: &'a World,
    detectors: &'a Detectors,
}

fn archive_path(data: &Path, epoch: u64) -> PathBuf {
    data.join("snapshots").join(format!("{epoch:08}.json"))
}

/// Loads an archived snapshot's world, for proofs of past epochs.
pub fn load_world(path: &Path) -> Result<World, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str::<Snapshot>(&text)
        .map(|s| s.world)
        .map_err(|e| format!("invalid snapshot {}: {e}", path.display()))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// When a window opened at `opened_ms` closes (spec §20): on the grid of whole epochs since the
/// Unix epoch (with 300 s, at :00, :05, :10…), at the first grid point that leaves the window at
/// least 2/5 of an epoch (120 s of 300). A late header moves the close to a later grid point; a
/// server that was down does not catch up.
pub fn close_after(opened_ms: u64, epoch_seconds: u64) -> u64 {
    let period = (epoch_seconds * 1000).max(1);
    let min_window = period * 2 / 5;
    (opened_ms + min_window).div_ceil(period) * period
}

/// Sleeps until a Unix time in milliseconds (in steps, so a clock set forward is followed).
fn sleep_until(at_ms: u64) {
    loop {
        let now = now_ms();
        if now >= at_ms {
            return;
        }
        std::thread::sleep(Duration::from_millis((at_ms - now).min(10_000)));
    }
}

/// Opens or creates the world and its log. Returns the run, its story detectors, the store and
/// the shared state for the API.
pub fn start(opts: Options) -> Result<(Run, Detectors, Store, Arc<Shared>), String> {
    std::fs::create_dir_all(opts.data.join("snapshots"))
        .map_err(|e| format!("cannot create {}: {e}", opts.data.display()))?;
    let mut store = Store::open(&opts.data.join(DB), opts.rules.clade_name_threshold)?;
    let latest = opts.data.join(LATEST);
    let (run, detectors, header) = match std::fs::read_to_string(&latest) {
        Ok(text) => {
            let s: Snapshot = serde_json::from_str(&text)
                .map_err(|e| format!("invalid snapshot {}: {e}", latest.display()))?;
            if s.seed != opts.seed {
                eprintln!(
                    "note: the saved world keeps its seed {} and ruleset",
                    s.seed
                );
            }
            println!("resuming seed {} at epoch {}", s.seed, s.world.epoch);
            store.rollback_after(&s.world)?;
            let run = Run::resume(s.seed, s.rules, s.world);
            let header = model::header(&run.world, &EpochReport::default(), &run.rules);
            (run, s.detectors, header)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("starting seed {} from genesis", opts.seed);
            store.clear()?;
            let run = Run::new(opts.seed, opts.rules.clone());
            let header = model::header(&run.world, &EpochReport::default(), &run.rules);
            store.set_meta("seed", &run.seed.to_string())?;
            store.set_meta(
                "ruleset",
                &serde_json::to_string(&run.rules).map_err(|e| e.to_string())?,
            )?;
            store.record_genesis(&run.world, &header)?;
            let detectors = Detectors::new();
            save(&opts.data, &run, &detectors, opts.archive_every)?;
            (run, detectors, header)
        }
        Err(e) => return Err(format!("cannot read {}: {e}", latest.display())),
    };
    let named = store.backfill_names()?;
    if named > 0 {
        println!("named {named} clades recorded before names were stored");
    }
    // The hourly dominant clade, from the last full hour's header (or the current one).
    let hour = run.world.epoch / DOMINANT_EVERY * DOMINANT_EVERY;
    let hour_dominant = crate::store::reader(&opts.data.join(DB))
        .and_then(|c| crate::store::header(&c, hour))?
        .and_then(|h| h["dominant_clade"].as_u64())
        .map_or(header.dominant_clade, |d| d as u32);
    let mut intake = Intake::open(
        &opts.data,
        run.world.world_id,
        run.world.ruleset_id,
        opts.price_min,
    )?;
    intake.rollback_after(run.world.epoch)?;
    if let Some(w) = opts.first_weight {
        intake.first_target = (u64::MAX / w.max(1)).min(i64::MAX as u64);
    }
    // The challenge commits to the last signed header (the state root before headers existed).
    let prev = intake
        .header_hash(run.world.epoch)?
        .unwrap_or_else(|| run.state_root());
    intake.open_window(run.world.epoch + 1, &prev)?;
    intake.soft_check(&run.world, &run.rules)?;
    // The chain of signed headers starts at genesis.
    if run.world.epoch == 0 && intake.header_hash(0)?.is_none() {
        intake.seal(0, run.world.ruleset_id, run.state_root(), [0; 32], 0)?;
    }
    let shared = Arc::new(Shared {
        intake: Mutex::new(intake),
        live: RwLock::new(Live {
            world: run.world.clone(),
            header,
            next_epoch_ms: close_after(now_ms(), opts.epoch_seconds),
            hour_dominant,
        }),
        seed: run.seed,
        rules: run.rules.clone(),
        data: opts.data,
        epoch_seconds: opts.epoch_seconds,
        archive_every: opts.archive_every.max(1),
        drand: opts.drand,
    });
    Ok((run, detectors, store, shared))
}

/// Runs forever: one epoch every `epoch_seconds`, its window closing on the grid of
/// `close_after`. World time is logical: after a delay the world does not catch up (spec §20).
pub fn run_loop(
    mut run: Run,
    mut detectors: Detectors,
    mut store: Store,
    shared: Arc<Shared>,
) -> Result<(), String> {
    loop {
        let close_at = shared
            .live
            .read()
            .expect("the lock is never poisoned")
            .next_epoch_ms;
        sleep_until(close_at);
        if run.world.finished(&run.rules) {
            shared
                .live
                .write()
                .expect("the lock is never poisoned")
                .next_epoch_ms = close_after(now_ms(), shared.epoch_seconds);
            continue;
        }
        let started = Instant::now();
        // The window of this epoch closes before the world steps into it (spec §20). Its round
        // is the first drand round at least 10 s later; the loop waits for it, then the ledger
        // selects the miracles (ties broken by the beacon) and the world steps, seeded by the
        // beacon and the previous signed header.
        let close_ms = now_ms();
        shared
            .intake
            .lock()
            .expect("the lock is never poisoned")
            .close_window()?;
        let (round, beacon) = if shared.drand {
            let round = protogaea_protocol::beacon::round_for_close(close_ms);
            (round, crate::beacon::wait_for(round))
        } else {
            (0, run.beacon(run.world.epoch))
        };
        let (selected, prev_header) = {
            let mut intake = shared.intake.lock().expect("the lock is never poisoned");
            let selected = intake.select_miracles(&beacon, &run.world, &run.rules)?;
            (
                selected,
                intake.header_hash(run.world.epoch)?.unwrap_or([0; 32]),
            )
        };
        let miracles: Vec<Miracle> = selected
            .iter()
            .map(|(_, w)| crate::intake::to_miracle(w, run.world.width))
            .collect();
        let seed = (round > 0).then_some((beacon, prev_header));
        let (header, outcomes) = step(
            &mut run,
            &mut detectors,
            &mut store,
            &shared,
            &miracles,
            seed,
        )?;
        shared
            .intake
            .lock()
            .expect("the lock is never poisoned")
            .record(run.world.epoch, &selected, &miracles, &outcomes)?;
        {
            let mut live = shared.live.write().expect("the lock is never poisoned");
            live.world = run.world.clone();
            if header.epoch.is_multiple_of(DOMINANT_EVERY) {
                live.hour_dominant = header.dominant_clade;
            }
            live.header = header;
        }
        let header_hash = shared
            .intake
            .lock()
            .expect("the lock is never poisoned")
            .seal(
                run.world.epoch,
                run.world.ruleset_id,
                run.state_root(),
                beacon,
                round,
            )?;
        save(&shared.data, &run, &detectors, shared.archive_every)?;
        shared
            .intake
            .lock()
            .expect("the lock is never poisoned")
            .open_window(run.world.epoch + 1, &header_hash)?;
        shared
            .live
            .write()
            .expect("the lock is never poisoned")
            .next_epoch_ms = close_after(now_ms(), shared.epoch_seconds);
        shared
            .intake
            .lock()
            .expect("the lock is never poisoned")
            .soft_check(&run.world, &run.rules)?;
        println!(
            "epoch {}: population {}, {} ms",
            run.world.epoch,
            run.world.organisms.len(),
            started.elapsed().as_millis()
        );
    }
}

/// One epoch: step, look for stories, then record it all.
pub fn step(
    run: &mut Run,
    detectors: &mut Detectors,
    store: &mut Store,
    shared: &Shared,
    miracles: &[Miracle],
    seed: Option<(protogaea_protocol::Hash, protogaea_protocol::Hash)>,
) -> Result<(Header, Outcomes), String> {
    let before = {
        let live = shared.live.read().expect("the lock is never poisoned");
        Before::of(&run.world, live.hour_dominant)
    };
    let max_id_before = run.world.organisms.last().map_or(0, |o| o.id);
    let report = match seed {
        Some((beacon, prev_header)) => run.step_seeded(miracles, &beacon, &prev_header),
        None => run.step_with(miracles),
    };
    let header = model::header(&run.world, &report, &run.rules);
    let mut events = model::events(&before, &run.world, &report, &header, &run.rules);
    events.extend(model::miracle_events(
        miracles,
        &report.miracles,
        run.world.width,
    ));
    let stories = detectors.observe(
        &run.world,
        &report,
        &Context {
            rules: &run.rules,
            plates: &run.plan.plates,
        },
    );

    // Every organism with a new id appeared this epoch, born or revived: the living ones and
    // those that died before the epoch ended.
    let mut newborn: Vec<_> = run
        .world
        .organisms
        .iter()
        .filter(|o| o.id > max_id_before)
        .copied()
        .chain(
            report
                .deaths_list
                .iter()
                .map(|(o, _)| *o)
                .filter(|o| o.id > max_id_before),
        )
        .collect();
    newborn.sort_by_key(|o| o.id);
    let founded = report
        .clades_founded
        .iter()
        .filter_map(|id| {
            run.world
                .clades
                .get(id)
                .or_else(|| report.clades_extinct.iter().find(|c| c.id == *id))
                .copied()
        })
        .collect();
    store.record_epoch(&EpochRecord {
        header: &header,
        events: &events,
        newborn,
        deaths: &report.deaths_list,
        world: &run.world,
        founded,
        extinct: &report.clades_extinct,
        stories: &stories,
    })?;
    Ok((header, report.miracles))
}

/// Replaces the latest snapshot atomically, and archives one every `archive_every` epochs.
fn save(data: &Path, run: &Run, detectors: &Detectors, archive_every: u64) -> Result<(), String> {
    let snapshot = SnapshotRef {
        seed: run.seed,
        rules: &run.rules,
        world: &run.world,
        detectors,
    };
    let json = serde_json::to_vec(&snapshot).map_err(|e| e.to_string())?;
    let tmp = data.join(format!("{LATEST}.tmp"));
    std::fs::write(&tmp, &json).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, data.join(LATEST))
        .map_err(|e| format!("cannot replace the snapshot: {e}"))?;
    if run.world.epoch.is_multiple_of(archive_every.max(1)) {
        let path = archive_path(data, run.world.epoch);
        std::fs::write(&path, &json)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_close_on_the_grid() {
        let m = 60_000;
        // Opened 15 s after :00, the window closes at :05 (285 s).
        assert_eq!(close_after(15_000, 300), 5 * m);
        // Opened at 3:30 it would last 90 s: the close moves to :10.
        assert_eq!(close_after(3 * m + 30_000, 300), 10 * m);
        // Exactly 120 s is enough.
        assert_eq!(close_after(3 * m, 300), 5 * m);
        // Back after a long stop: the next grid point, no catching up.
        assert_eq!(close_after(1000 * m + 1, 300), 1005 * m);
        assert_eq!(close_after(7, 0), 7);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("protogaea-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn options(data: PathBuf) -> Options {
        Options {
            seed: 3,
            rules: Ruleset::default(),
            data,
            epoch_seconds: 0,
            archive_every: 4,
            price_min: 1_000_000,
            drand: false,
            first_weight: None,
        }
    }

    fn advance(
        run: &mut Run,
        detectors: &mut Detectors,
        store: &mut Store,
        shared: &Shared,
        epochs: u32,
        snapshot: bool,
    ) {
        for _ in 0..epochs {
            let (header, _) =
                step(run, detectors, store, shared, &[], None).expect("the epoch is recorded");
            {
                let mut live = shared.live.write().unwrap();
                if header.epoch.is_multiple_of(DOMINANT_EVERY) {
                    live.hour_dominant = header.dominant_clade;
                }
                live.header = header;
            }
            if snapshot {
                save(&shared.data, run, detectors, shared.archive_every)
                    .expect("the snapshot is saved");
            }
        }
    }

    fn counts(data: &Path) -> Vec<i64> {
        let c = crate::store::reader(&data.join(DB)).unwrap();
        [
            "SELECT count(*) FROM epochs",
            "SELECT count(*) FROM stories",
            "SELECT count(*) FROM events",
            "SELECT count(*) FROM organisms",
            "SELECT count(*) FROM organisms WHERE died_epoch IS NOT NULL",
            "SELECT count(*) FROM clades",
            "SELECT count(*) FROM clades WHERE extinct_epoch IS NOT NULL",
        ]
        .iter()
        .map(|q| c.query_row(q, [], |r| r.get(0)).unwrap())
        .collect()
    }

    /// A crash after the database write and before the snapshot loses nothing: the restart rolls
    /// the log back to the snapshot and recomputes the same epochs.
    #[test]
    fn a_restart_after_a_crash_matches_an_uninterrupted_run() {
        let clean = temp_dir("clean");
        let (mut run, mut det, mut store, shared) = start(options(clean.clone())).unwrap();
        advance(&mut run, &mut det, &mut store, &shared, 8, true);
        let clean_root = run.state_root();
        drop(store);

        let crashed = temp_dir("crashed");
        let (mut run, mut det, mut store, shared) = start(options(crashed.clone())).unwrap();
        advance(&mut run, &mut det, &mut store, &shared, 5, true);
        advance(&mut run, &mut det, &mut store, &shared, 2, false); // recorded, but no snapshot
        drop(store);
        let (mut run, mut det, mut store, shared) = start(options(crashed.clone())).unwrap();
        assert_eq!(run.world.epoch, 5);
        advance(&mut run, &mut det, &mut store, &shared, 3, true);

        assert_eq!(run.state_root(), clean_root);
        assert_eq!(counts(&crashed), counts(&clean));
        let _ = std::fs::remove_dir_all(clean);
        let _ = std::fs::remove_dir_all(crashed);
    }
}
