//! The world loop: advances the authoritative world on a timer and records every epoch.
//!
//! Order per epoch: step the world, write the epoch to the database in one transaction, then
//! replace the latest snapshot. After a crash between the two writes, the restart drops the
//! database rows past the snapshot and recomputes them; the world is deterministic, so they come
//! out the same.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use protogaea_core::run::Run;
use protogaea_core::{EpochReport, Ruleset, World};
use serde::{Deserialize, Serialize};

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
    pub seed: u64,
    pub rules: Ruleset,
    pub data: PathBuf,
    pub epoch_seconds: u64,
    pub archive_every: u64,
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
}

#[derive(Serialize)]
struct SnapshotRef<'a> {
    seed: u64,
    rules: &'a Ruleset,
    world: &'a World,
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

/// Opens or creates the world and its log. Returns the run and the shared state for the API.
pub fn start(opts: Options) -> Result<(Run, Store, Arc<Shared>), String> {
    std::fs::create_dir_all(opts.data.join("snapshots"))
        .map_err(|e| format!("cannot create {}: {e}", opts.data.display()))?;
    let mut store = Store::open(&opts.data.join(DB))?;
    let latest = opts.data.join(LATEST);
    let (run, header) = match std::fs::read_to_string(&latest) {
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
            (run, header)
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
            save(&opts.data, &run, opts.archive_every)?;
            (run, header)
        }
        Err(e) => return Err(format!("cannot read {}: {e}", latest.display())),
    };
    // The hourly dominant clade, from the last full hour's header (or the current one).
    let hour = run.world.epoch / DOMINANT_EVERY * DOMINANT_EVERY;
    let hour_dominant = crate::store::reader(&opts.data.join(DB))
        .and_then(|c| crate::store::header(&c, hour))?
        .and_then(|h| h["dominant_clade"].as_u64())
        .map_or(header.dominant_clade, |d| d as u32);
    let shared = Arc::new(Shared {
        live: RwLock::new(Live {
            world: run.world.clone(),
            header,
            next_epoch_ms: now_ms() + opts.epoch_seconds * 1000,
            hour_dominant,
        }),
        seed: run.seed,
        rules: run.rules.clone(),
        data: opts.data,
        epoch_seconds: opts.epoch_seconds,
        archive_every: opts.archive_every.max(1),
    });
    Ok((run, store, shared))
}

/// Runs forever: one epoch every `epoch_seconds`. World time is logical: after a delay the
/// world does not catch up (spec §20).
pub fn run_loop(mut run: Run, mut store: Store, shared: Arc<Shared>) -> Result<(), String> {
    let period = Duration::from_secs(shared.epoch_seconds);
    let mut next = Instant::now() + period;
    loop {
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        }
        next = Instant::now().max(next) + period;
        if run.world.finished(&run.rules) {
            continue;
        }
        let started = Instant::now();
        let header = step(&mut run, &mut store, &shared)?;
        {
            let mut live = shared.live.write().expect("the lock is never poisoned");
            live.world = run.world.clone();
            if header.epoch.is_multiple_of(DOMINANT_EVERY) {
                live.hour_dominant = header.dominant_clade;
            }
            live.header = header;
            live.next_epoch_ms = now_ms() + period.as_millis() as u64;
        }
        save(&shared.data, &run, shared.archive_every)?;
        println!(
            "epoch {}: population {}, {} ms",
            run.world.epoch,
            run.world.organisms.len(),
            started.elapsed().as_millis()
        );
    }
}

/// One epoch: step, then record it.
pub fn step(run: &mut Run, store: &mut Store, shared: &Shared) -> Result<Header, String> {
    let before = {
        let live = shared.live.read().expect("the lock is never poisoned");
        Before::of(&run.world, live.hour_dominant)
    };
    let max_id_before = run.world.organisms.last().map_or(0, |o| o.id);
    let report = run.step();
    let header = model::header(&run.world, &report, &run.rules);
    let events = model::events(&before, &run.world, &report, &header, &run.rules);

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
    })?;
    Ok(header)
}

/// Replaces the latest snapshot atomically, and archives one every `archive_every` epochs.
fn save(data: &Path, run: &Run, archive_every: u64) -> Result<(), String> {
    let snapshot = SnapshotRef {
        seed: run.seed,
        rules: &run.rules,
        world: &run.world,
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
        }
    }

    fn advance(run: &mut Run, store: &mut Store, shared: &Shared, epochs: u32, snapshot: bool) {
        for _ in 0..epochs {
            let header = step(run, store, shared).expect("the epoch is recorded");
            {
                let mut live = shared.live.write().unwrap();
                if header.epoch.is_multiple_of(DOMINANT_EVERY) {
                    live.hour_dominant = header.dominant_clade;
                }
                live.header = header;
            }
            if snapshot {
                save(&shared.data, run, shared.archive_every).expect("the snapshot is saved");
            }
        }
    }

    fn counts(data: &Path) -> Vec<i64> {
        let c = crate::store::reader(&data.join(DB)).unwrap();
        [
            "SELECT count(*) FROM epochs",
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
        let (mut run, mut store, shared) = start(options(clean.clone())).unwrap();
        advance(&mut run, &mut store, &shared, 8, true);
        let clean_root = run.state_root();
        drop(store);

        let crashed = temp_dir("crashed");
        let (mut run, mut store, shared) = start(options(crashed.clone())).unwrap();
        advance(&mut run, &mut store, &shared, 5, true);
        advance(&mut run, &mut store, &shared, 2, false); // recorded, but no snapshot
        drop(store);
        let (mut run, mut store, shared) = start(options(crashed.clone())).unwrap();
        assert_eq!(run.world.epoch, 5);
        advance(&mut run, &mut store, &shared, 3, true);

        assert_eq!(run.state_root(), clean_root);
        assert_eq!(counts(&crashed), counts(&clean));
        let _ = std::fs::remove_dir_all(clean);
        let _ = std::fs::remove_dir_all(crashed);
    }
}
