//! The event log and indexes in SQLite: epoch headers, events, clades, organisms (the dead
//! included) and clade counts for the Muller plot. Everything here can be rebuilt from the world;
//! the world itself lives in snapshot files.

use std::path::Path;

use protogaea_core::{Clade, DeathCause, Organism, World};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde_json::{json, Value};

use protogaea_stories::Story;

use crate::model::{Event, Header};
use crate::names;

/// Clade counts for the Muller plot are kept once per world hour.
pub const MULLER_EVERY: u64 = 12;

/// Replay frames are kept for this many recent epochs (two world days).
pub const FRAMES_KEPT: u64 = 576;
/// Bytes per organism in a frame: id (low 32 bits), cell, clade, hue, archetype.
pub const FRAME_ORGANISM: usize = 13;

const SCHEMA: &str = "
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS epochs (
    epoch INTEGER PRIMARY KEY,
    state_root TEXT NOT NULL,
    header TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    epoch INTEGER NOT NULL,
    kind TEXT NOT NULL,
    clade_id INTEGER,
    organism_id INTEGER,
    data TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS events_epoch ON events (epoch);
CREATE INDEX IF NOT EXISTS events_clade ON events (clade_id);
CREATE TABLE IF NOT EXISTS clades (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER NOT NULL,
    founded_epoch INTEGER NOT NULL,
    extinct_epoch INTEGER,
    living INTEGER NOT NULL,
    peak_living INTEGER NOT NULL,
    reference TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS clades_parent ON clades (parent_id);
CREATE TABLE IF NOT EXISTS organisms (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER NOT NULL,
    clade_id INTEGER NOT NULL,
    lineage_id INTEGER NOT NULL,
    born_epoch INTEGER NOT NULL,
    died_epoch INTEGER,
    cause TEXT,
    genome TEXT NOT NULL,
    at_death TEXT
);
CREATE INDEX IF NOT EXISTS organisms_parent ON organisms (parent_id);
CREATE INDEX IF NOT EXISTS organisms_clade ON organisms (clade_id);
CREATE TABLE IF NOT EXISTS frames (
    epoch INTEGER PRIMARY KEY,
    data BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS stories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    epoch INTEGER NOT NULL,
    kind TEXT NOT NULL,
    clade_id INTEGER,
    other_id INTEGER,
    plate INTEGER,
    score INTEGER NOT NULL,
    data TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS stories_epoch ON stories (epoch);
CREATE TABLE IF NOT EXISTS muller (
    epoch INTEGER NOT NULL,
    clade_id INTEGER NOT NULL,
    living INTEGER NOT NULL,
    PRIMARY KEY (epoch, clade_id)
);
";

type Result<T> = std::result::Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    format!("database: {e}")
}

/// Everything recorded about one epoch, written in a single transaction.
pub struct EpochRecord<'a> {
    pub header: &'a Header,
    pub events: &'a [Event],
    /// Organisms that appeared this epoch, born or revived, as they were when first seen.
    pub newborn: Vec<Organism>,
    pub deaths: &'a [(Organism, DeathCause)],
    pub world: &'a World,
    pub founded: Vec<Clade>,
    pub extinct: &'a [Clade],
    pub stories: &'a [Story],
}

pub struct Store {
    conn: Connection,
    name_threshold: u32,
}

impl Store {
    pub fn open(path: &Path, name_threshold: u32) -> Result<Self> {
        let conn = Connection::open(path).map_err(err)?;
        conn.execute_batch(SCHEMA).map_err(err)?;
        // Columns added after the first worlds were recorded; adding one twice is harmless.
        for column in ["name TEXT", "combo INTEGER", "named_epoch INTEGER"] {
            let _ = conn.execute(&format!("ALTER TABLE clades ADD COLUMN {column}"), []);
        }
        conn.execute_batch("CREATE INDEX IF NOT EXISTS clades_combo ON clades (combo);")
            .map_err(err)?;
        Ok(Self {
            conn,
            name_threshold,
        })
    }

    /// Names the clades that have reached the threshold and have no name yet, oldest first
    /// (by when they were named in the event log, else when they were founded). For worlds
    /// recorded before names were stored, and after a rollback.
    pub fn backfill_names(&mut self) -> Result<usize> {
        let tx = self.conn.transaction().map_err(err)?;
        let pending: Vec<(u32, String, i64)> = {
            let mut stmt = tx
                .prepare(
                    "SELECT c.id, c.reference, COALESCE(
                         (SELECT min(e.epoch) FROM events e WHERE e.kind = 'clade_named' AND e.clade_id = c.id),
                         c.founded_epoch) AS named
                     FROM clades c WHERE c.name IS NULL AND c.peak_living >= ?1 ORDER BY named, c.id",
                )
                .map_err(err)?;
            let rows = stmt
                .query_map([self.name_threshold], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .map_err(err)?;
            rows.collect::<rusqlite::Result<_>>().map_err(err)?
        };
        for (id, reference, epoch) in &pending {
            if let Ok(genome) = serde_json::from_str(reference) {
                give_name(&tx, *id, &genome, *epoch as u64)?;
            }
        }
        tx.commit().map_err(err)?;
        Ok(pending.len())
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                [key, value],
            )
            .map(drop)
            .map_err(err)
    }

    /// Forgets everything; used when there is no snapshot to resume from.
    pub fn clear(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "DELETE FROM epochs; DELETE FROM events; DELETE FROM clades; DELETE FROM stories; DELETE FROM frames;
                 DELETE FROM organisms; DELETE FROM muller; DELETE FROM meta;",
            )
            .map_err(err)
    }

    /// Genesis: the founders, their clades and the header of epoch 0.
    pub fn record_genesis(&mut self, world: &World, header: &Header) -> Result<()> {
        let tx = self.conn.transaction().map_err(err)?;
        insert_header(&tx, header)?;
        for c in world.clades.values() {
            insert_clade(&tx, c)?;
        }
        name_new(&tx, world.clades.values(), 0, self.name_threshold)?;
        for o in &world.organisms {
            insert_organism(&tx, o, 0)?;
        }
        insert_muller(&tx, world)?;
        tx.commit().map_err(err)
    }

    pub fn record_epoch(&mut self, rec: &EpochRecord) -> Result<()> {
        let epoch = rec.header.epoch;
        let tx = self.conn.transaction().map_err(err)?;
        insert_header(&tx, rec.header)?;
        {
            let mut stmt = tx
                .prepare_cached(
                    "INSERT INTO events (epoch, kind, clade_id, organism_id, data)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .map_err(err)?;
            for e in rec.events {
                stmt.execute(params![
                    epoch as i64,
                    e.kind,
                    e.clade_id,
                    e.organism_id.map(|id| id as i64),
                    e.data.to_string()
                ])
                .map_err(err)?;
            }
        }
        for c in &rec.founded {
            insert_clade(&tx, c)?;
        }
        {
            let mut stmt = tx
                .prepare_cached("UPDATE clades SET living = ?2, peak_living = ?3 WHERE id = ?1")
                .map_err(err)?;
            for c in rec.world.clades.values() {
                stmt.execute(params![c.id, c.living, c.peak_living])
                    .map_err(err)?;
            }
        }
        for c in rec.extinct {
            tx.execute(
                "UPDATE clades SET living = 0, peak_living = ?2, extinct_epoch = ?3 WHERE id = ?1",
                params![c.id, c.peak_living, epoch as i64],
            )
            .map_err(err)?;
        }
        name_new(
            &tx,
            rec.world.clades.values().chain(rec.extinct.iter()),
            epoch,
            self.name_threshold,
        )?;
        for o in &rec.newborn {
            insert_organism(&tx, o, epoch)?;
        }
        {
            let mut stmt = tx
                .prepare_cached(
                    "UPDATE organisms SET died_epoch = ?2, cause = ?3, at_death = ?4 WHERE id = ?1",
                )
                .map_err(err)?;
            for (o, cause) in rec.deaths {
                stmt.execute(params![
                    o.id as i64,
                    epoch as i64,
                    cause_name(*cause),
                    json!({ "age": o.age, "energy": o.energy, "cell": o.cell }).to_string()
                ])
                .map_err(err)?;
            }
        }
        {
            let mut stmt = tx
                .prepare_cached(
                    "INSERT INTO stories (epoch, kind, clade_id, other_id, plate, score, data)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                )
                .map_err(err)?;
            for s in rec.stories {
                stmt.execute(params![
                    epoch as i64,
                    s.kind.name(),
                    s.clade,
                    s.other,
                    s.plate,
                    s.score,
                    s.data.to_string()
                ])
                .map_err(err)?;
            }
        }
        if epoch.is_multiple_of(MULLER_EVERY) {
            insert_muller(&tx, rec.world)?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO frames (epoch, data) VALUES (?1, ?2)",
            params![epoch as i64, frame(rec.world)],
        )
        .map_err(err)?;
        tx.execute(
            "DELETE FROM frames WHERE epoch < ?1",
            [epoch.saturating_sub(FRAMES_KEPT) as i64],
        )
        .map_err(err)?;
        tx.commit().map_err(err)
    }

    /// After a restart from a snapshot of `world`, drops whatever was recorded for later epochs;
    /// the world recomputes them identically.
    pub fn rollback_after(&mut self, world: &World) -> Result<()> {
        let e = world.epoch as i64;
        let tx = self.conn.transaction().map_err(err)?;
        tx.execute_batch(&format!(
            "DELETE FROM epochs WHERE epoch > {e};
             DELETE FROM events WHERE epoch > {e};
             DELETE FROM stories WHERE epoch > {e};
             DELETE FROM frames WHERE epoch > {e};
             DELETE FROM muller WHERE epoch > {e};
             DELETE FROM organisms WHERE born_epoch > {e};
             UPDATE organisms SET died_epoch = NULL, cause = NULL, at_death = NULL WHERE died_epoch > {e};
             DELETE FROM clades WHERE founded_epoch > {e};
             UPDATE clades SET extinct_epoch = NULL WHERE extinct_epoch > {e};
             UPDATE clades SET name = NULL, combo = NULL, named_epoch = NULL WHERE named_epoch > {e};"
        ))
        .map_err(err)?;
        for c in world.clades.values() {
            tx.execute(
                "UPDATE clades SET living = ?2, peak_living = ?3 WHERE id = ?1",
                params![c.id, c.living, c.peak_living],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)
    }
}

/// Names every clade among `clades` that has reached the threshold and has no name yet.
fn name_new<'a>(
    tx: &Connection,
    clades: impl Iterator<Item = &'a Clade>,
    epoch: u64,
    threshold: u32,
) -> Result<()> {
    let mut candidates: Vec<&Clade> = clades.filter(|c| c.peak_living >= threshold).collect();
    candidates.sort_by_key(|c| c.id);
    for c in candidates {
        let unnamed: Option<bool> = tx
            .prepare_cached("SELECT name IS NULL FROM clades WHERE id = ?1")
            .and_then(|mut s| s.query_row([c.id], |r| r.get(0)).optional())
            .map_err(err)?;
        if unnamed == Some(true) {
            give_name(tx, c.id, &c.reference, epoch)?;
        }
    }
    Ok(())
}

/// The next free name in the clade's combination.
fn give_name(
    tx: &Connection,
    id: u32,
    reference: &protogaea_core::Genome,
    epoch: u64,
) -> Result<()> {
    let combo = names::combo(reference);
    let taken: u32 = tx
        .prepare_cached("SELECT count(*) FROM clades WHERE combo = ?1")
        .and_then(|mut s| s.query_row([combo], |r| r.get(0)))
        .map_err(err)?;
    tx.prepare_cached("UPDATE clades SET name = ?2, combo = ?3, named_epoch = ?4 WHERE id = ?1")
        .and_then(|mut s| {
            s.execute(params![
                id,
                names::name_in(combo, taken),
                combo,
                epoch as i64
            ])
        })
        .map(drop)
        .map_err(err)
}

/// A compact picture of where every organism stands, for the replay: per organism the low 32 bits
/// of its id, its cell, clade, hue and archetype, little-endian.
fn frame(world: &World) -> Vec<u8> {
    let mut out = Vec::with_capacity(world.organisms.len() * FRAME_ORGANISM);
    for o in &world.organisms {
        out.extend_from_slice(&(o.id as u32).to_le_bytes());
        out.extend_from_slice(&o.cell.to_le_bytes());
        out.extend_from_slice(&o.clade_id.to_le_bytes());
        out.extend_from_slice(&o.genome.hue.to_le_bytes());
        out.push(crate::model::archetype(&o.genome.traits));
    }
    out
}

fn cause_name(cause: DeathCause) -> &'static str {
    match cause {
        DeathCause::Starvation => "starvation",
        DeathCause::OldAge => "old_age",
        DeathCause::Predation => "predation",
        DeathCause::Plague => "plague",
        DeathCause::Drowned => "drowned",
    }
}

fn insert_header(tx: &Connection, h: &Header) -> Result<()> {
    tx.execute(
        "INSERT OR REPLACE INTO epochs (epoch, state_root, header) VALUES (?1, ?2, ?3)",
        params![
            h.epoch as i64,
            h.state_root,
            serde_json::to_string(h).map_err(err)?
        ],
    )
    .map(drop)
    .map_err(err)
}

fn insert_clade(tx: &Connection, c: &Clade) -> Result<()> {
    tx.execute(
        "INSERT INTO clades (id, parent_id, founded_epoch, extinct_epoch, living, peak_living, reference)
         VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6)
         ON CONFLICT (id) DO UPDATE SET living = excluded.living, peak_living = excluded.peak_living",
        params![
            c.id,
            c.parent_id,
            c.founded_epoch as i64,
            c.living,
            c.peak_living,
            serde_json::to_string(&c.reference).map_err(err)?
        ],
    )
    .map(drop)
    .map_err(err)
}

fn insert_organism(tx: &Connection, o: &Organism, epoch: u64) -> Result<()> {
    tx.prepare_cached(
        "INSERT OR REPLACE INTO organisms (id, parent_id, clade_id, lineage_id, born_epoch, genome)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .and_then(|mut stmt| {
        stmt.execute(params![
            o.id as i64,
            o.parent_id as i64,
            o.clade_id,
            o.lineage_id,
            epoch as i64,
            serde_json::to_string(&o.genome).unwrap_or_default()
        ])
    })
    .map(drop)
    .map_err(err)
}

fn insert_muller(tx: &Connection, world: &World) -> Result<()> {
    let mut stmt = tx
        .prepare_cached(
            "INSERT OR REPLACE INTO muller (epoch, clade_id, living) VALUES (?1, ?2, ?3)",
        )
        .map_err(err)?;
    for c in world.clades.values().filter(|c| c.living > 0) {
        stmt.execute(params![world.epoch as i64, c.id, c.living])
            .map_err(err)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- reads, for the API

/// A read-only connection; the API opens one per request, which is cheap with SQLite.
pub fn reader(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(err)
}

fn json_col(text: String) -> Value {
    serde_json::from_str(&text).unwrap_or(Value::Null)
}

pub fn header(conn: &Connection, epoch: u64) -> Result<Option<Value>> {
    conn.query_row(
        "SELECT header FROM epochs WHERE epoch = ?1",
        [epoch as i64],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map(|h| h.map(json_col))
    .map_err(err)
}

/// Headers from `from` to `to`, every `step`-th epoch, at most `limit` of them.
pub fn headers(conn: &Connection, from: u64, to: u64, step: u64, limit: u32) -> Result<Vec<Value>> {
    let mut stmt = conn
        .prepare(
            "SELECT header FROM epochs WHERE epoch BETWEEN ?1 AND ?2 AND epoch % ?3 = 0
             ORDER BY epoch LIMIT ?4",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map(
            params![from as i64, to as i64, step.max(1) as i64, limit],
            |r| r.get::<_, String>(0),
        )
        .map_err(err)?;
    rows.map(|r| r.map(json_col).map_err(err)).collect()
}

fn event_row(r: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, i64>(0)?,
        "epoch": r.get::<_, i64>(1)?,
        "kind": r.get::<_, String>(2)?,
        "clade_id": r.get::<_, Option<i64>>(3)?,
        "organism_id": r.get::<_, Option<i64>>(4)?,
        "data": json_col(r.get::<_, String>(5)?),
    }))
}

/// Events after the cursor (an event id), oldest first; optionally of one kind or one clade.
pub fn events(
    conn: &Connection,
    cursor: i64,
    limit: u32,
    kind: Option<&str>,
    clade: Option<u32>,
) -> Result<Vec<Value>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, epoch, kind, clade_id, organism_id, data FROM events
             WHERE id > ?1 AND (?2 IS NULL OR kind = ?2) AND (?3 IS NULL OR clade_id = ?3)
             ORDER BY id LIMIT ?4",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map(params![cursor, kind, clade, limit], event_row)
        .map_err(err)?;
    rows.map(|r| r.map_err(err)).collect()
}

/// The newest events older than `before` (any, if 0), newest first.
pub fn events_before(
    conn: &Connection,
    before: i64,
    limit: u32,
    kind: Option<&str>,
    clade: Option<u32>,
    until: Option<u64>,
) -> Result<Vec<Value>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, epoch, kind, clade_id, organism_id, data FROM events
             WHERE (?1 = 0 OR id < ?1) AND (?2 IS NULL OR kind = ?2) AND (?3 IS NULL OR clade_id = ?3)
               AND (?5 IS NULL OR epoch <= ?5)
             ORDER BY id DESC LIMIT ?4",
        )
        .map_err(err)?;
    let until = until.map(|u| u.min(i64::MAX as u64) as i64);
    let rows = stmt
        .query_map(params![before, kind, clade, limit, until], event_row)
        .map_err(err)?;
    rows.map(|r| r.map_err(err)).collect()
}

fn clade_row(r: &rusqlite::Row) -> rusqlite::Result<Value> {
    Ok(json!({
        "id": r.get::<_, i64>(0)?,
        "parent_id": r.get::<_, i64>(1)?,
        "founded_epoch": r.get::<_, i64>(2)?,
        "extinct_epoch": r.get::<_, Option<i64>>(3)?,
        "living": r.get::<_, i64>(4)?,
        "peak_living": r.get::<_, i64>(5)?,
        "reference": json_col(r.get::<_, String>(6)?),
        "name": r.get::<_, Option<String>>(7)?,
    }))
}

const CLADE_COLUMNS: &str =
    "id, parent_id, founded_epoch, extinct_epoch, living, peak_living, reference, name";

pub fn clade(conn: &Connection, id: u32) -> Result<Option<Value>> {
    let Some(mut c) = conn
        .query_row(
            &format!("SELECT {CLADE_COLUMNS} FROM clades WHERE id = ?1"),
            [id],
            clade_row,
        )
        .optional()
        .map_err(err)?
    else {
        return Ok(None);
    };
    let mut stmt = conn
        .prepare("SELECT id FROM clades WHERE parent_id = ?1 AND id != ?1 ORDER BY id")
        .map_err(err)?;
    let children: Vec<i64> = stmt
        .query_map([id], |r| r.get(0))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    let mut stmt = conn
        .prepare("SELECT epoch, living FROM muller WHERE clade_id = ?1 ORDER BY epoch")
        .map_err(err)?;
    let history: Vec<[i64; 2]> = stmt
        .query_map([id], |r| Ok([r.get(0)?, r.get(1)?]))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    c["children"] = json!(children);
    c["history"] = json!(history);
    Ok(Some(c))
}

/// Clades by id; `living` limits the list to clades with members, `named` to those that reached
/// the naming threshold.
pub fn clades(conn: &Connection, living: bool, min_peak: u32, limit: u32) -> Result<Vec<Value>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {CLADE_COLUMNS} FROM clades
             WHERE (?1 = 0 OR living > 0) AND peak_living >= ?2 ORDER BY id LIMIT ?3"
        ))
        .map_err(err)?;
    let rows = stmt
        .query_map(params![living as i64, min_peak, limit], clade_row)
        .map_err(err)?;
    rows.map(|r| r.map_err(err)).collect()
}

/// Extinct clades that were named, most recent first (spec §7, §12).
pub fn museum(conn: &Connection, min_peak: u32, limit: u32) -> Result<Vec<Value>> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {CLADE_COLUMNS} FROM clades
             WHERE extinct_epoch IS NOT NULL AND peak_living >= ?1
             ORDER BY extinct_epoch DESC, id LIMIT ?2"
        ))
        .map_err(err)?;
    let rows = stmt
        .query_map(params![min_peak, limit], clade_row)
        .map_err(err)?;
    rows.map(|r| r.map_err(err)).collect()
}

pub fn organism(conn: &Connection, id: u64) -> Result<Option<Value>> {
    let Some(mut o) = conn
        .query_row(
            "SELECT id, parent_id, clade_id, lineage_id, born_epoch, died_epoch, cause, genome, at_death
             FROM organisms WHERE id = ?1",
            [id as i64],
            |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "parent_id": r.get::<_, i64>(1)?,
                    "clade_id": r.get::<_, i64>(2)?,
                    "lineage_id": r.get::<_, i64>(3)?,
                    "born_epoch": r.get::<_, i64>(4)?,
                    "died_epoch": r.get::<_, Option<i64>>(5)?,
                    "cause": r.get::<_, Option<String>>(6)?,
                    "genome": json_col(r.get::<_, String>(7)?),
                    "at_death": r.get::<_, Option<String>>(8)?.map(json_col),
                }))
            },
        )
        .optional()
        .map_err(err)?
    else {
        return Ok(None);
    };
    let mut stmt = conn
        .prepare("SELECT id FROM organisms WHERE parent_id = ?1 ORDER BY id LIMIT 200")
        .map_err(err)?;
    let offspring: Vec<i64> = stmt
        .query_map([id as i64], |r| r.get(0))
        .map_err(err)?
        .collect::<rusqlite::Result<_>>()
        .map_err(err)?;
    o["offspring"] = json!(offspring);
    Ok(Some(o))
}

/// The stored names of the given clades; unnamed clades are left out.
pub fn clade_names(conn: &Connection, ids: &[u32]) -> Result<Vec<(u32, String)>> {
    let mut stmt = conn
        .prepare_cached("SELECT name FROM clades WHERE id = ?1 AND name IS NOT NULL")
        .map_err(err)?;
    let mut out = Vec::with_capacity(ids.len());
    for &id in ids {
        if let Some(name) = stmt
            .query_row([id], |r| r.get::<_, String>(0))
            .optional()
            .map_err(err)?
        {
            out.push((id, name));
        }
    }
    Ok(out)
}

/// One row per clade of the season, for the clade tree and the Muller plot:
/// `(id, parent, founded, extinct, peak, reference JSON, name)`.
pub type TreeRow = (u32, u32, i64, Option<i64>, u32, String, Option<String>);

pub fn tree(conn: &Connection) -> Result<Vec<TreeRow>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, parent_id, founded_epoch, extinct_epoch, peak_living, reference, name
             FROM clades ORDER BY id",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .map_err(err)?;
    rows.map(|r| r.map_err(err)).collect()
}

/// Stories from epoch `since` to `until`, the most important first, then the newest.
pub fn stories(conn: &Connection, since: u64, until: u64, limit: u32) -> Result<Vec<Value>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, epoch, kind, clade_id, other_id, plate, score, data FROM stories
             WHERE epoch >= ?1 AND epoch <= ?2 ORDER BY score DESC, epoch DESC, id LIMIT ?3",
        )
        .map_err(err)?;
    let until = until.min(i64::MAX as u64) as i64;
    let rows = stmt
        .query_map(params![since as i64, until, limit], |r| {
            Ok(json!({
                "id": r.get::<_, i64>(0)?,
                "epoch": r.get::<_, i64>(1)?,
                "kind": r.get::<_, String>(2)?,
                "clade_id": r.get::<_, Option<i64>>(3)?,
                "other_id": r.get::<_, Option<i64>>(4)?,
                "plate": r.get::<_, Option<i64>>(5)?,
                "score": r.get::<_, i64>(6)?,
                "data": json_col(r.get::<_, String>(7)?),
            }))
        })
        .map_err(err)?;
    rows.map(|r| r.map_err(err)).collect()
}

/// Replay frames from `from` to `to`, every `step`-th epoch: `(epoch, bytes)`.
pub fn frames(conn: &Connection, from: u64, to: u64, step: u64) -> Result<Vec<(u64, Vec<u8>)>> {
    let mut stmt = conn
        .prepare(
            "SELECT epoch, data FROM frames WHERE epoch BETWEEN ?1 AND ?2 AND epoch % ?3 = 0 ORDER BY epoch",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map(params![from as i64, to as i64, step.max(1) as i64], |r| {
            Ok((r.get::<_, i64>(0)? as u64, r.get::<_, Vec<u8>>(1)?))
        })
        .map_err(err)?;
    rows.map(|r| r.map_err(err)).collect()
}

/// How many events of each kind happened after `since`, up to `until`.
pub fn event_counts(conn: &Connection, since: u64, until: u64) -> Result<Vec<(String, i64)>> {
    let mut stmt = conn
        .prepare("SELECT kind, count(*) FROM events WHERE epoch > ?1 AND epoch <= ?2 GROUP BY kind")
        .map_err(err)?;
    let rows = stmt
        .query_map(params![since as i64, until as i64], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(err)?;
    rows.map(|r| r.map_err(err)).collect()
}

/// Events of one kind after `since`, up to `until`, oldest first.
pub fn events_since(conn: &Connection, since: u64, until: u64, kind: &str) -> Result<Vec<Value>> {
    let mut stmt = conn
        .prepare(
            "SELECT id, epoch, kind, clade_id, organism_id, data FROM events
             WHERE epoch > ?1 AND epoch <= ?2 AND kind = ?3 ORDER BY id LIMIT 200",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map(params![since as i64, until as i64, kind], event_row)
        .map_err(err)?;
    rows.map(|r| r.map_err(err)).collect()
}

/// Clade counts over time for the Muller plot: `[epoch, clade, living]` from `from` on.
pub fn muller(conn: &Connection, from: u64, step: u64) -> Result<Vec<[i64; 3]>> {
    let mut stmt = conn
        .prepare(
            "SELECT epoch, clade_id, living FROM muller
             WHERE epoch >= ?1 AND epoch % ?2 = 0 ORDER BY epoch, clade_id",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map(
            params![from as i64, (step.max(1) * MULLER_EVERY) as i64],
            |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?]),
        )
        .map_err(err)?;
    rows.map(|r| r.map_err(err)).collect()
}
