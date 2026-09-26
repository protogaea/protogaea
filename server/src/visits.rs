//! Anonymous visit counts for the early tests (roadmap B6): a random id per browser, what it did and
//! when. No addresses, no user agents, nothing about the person. Kept in a file of its own, apart
//! from the world's data, so that a world can be restarted or rolled back without touching it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection};
use serde_json::{json, Value};

pub const DB: &str = "visits.sqlite";

/// What the viewer reports.
pub const KINDS: &[&str] = &[
    "visit",      // the viewer opened
    "digest",     // "While you were away" shown
    "story",      // a link in a story followed
    "card",       // a clade or an organism card opened
    "prediction", // a prediction made
    "replay",     // the last day replayed
    "view",       // the Muller plot or the clade tree opened
];

/// At most this many records per visitor per hour; the rest are dropped.
const PER_HOUR: i64 = 300;
const DAY: i64 = 86_400;

fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(err)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(err)?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         CREATE TABLE IF NOT EXISTS visits (
             visitor TEXT NOT NULL,
             at INTEGER NOT NULL,
             kind TEXT NOT NULL,
             detail TEXT
         );
         CREATE INDEX IF NOT EXISTS visits_by_visitor ON visits (visitor, at);",
    )
    .map_err(err)?;
    Ok(conn)
}

/// A visitor id is what the viewer makes: 16 to 64 hexadecimal digits.
pub fn valid_visitor(id: &str) -> bool {
    (16..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Records one thing a visitor did at `at` (Unix seconds). Returns false when it was dropped.
pub fn record(
    conn: &Connection,
    visitor: &str,
    kind: &str,
    detail: Option<&str>,
    at: i64,
) -> Result<bool, String> {
    if !valid_visitor(visitor) || !KINDS.contains(&kind) {
        return Ok(false);
    }
    let recent: i64 = conn
        .query_row(
            "SELECT count(*) FROM visits WHERE visitor = ?1 AND at > ?2",
            params![visitor, at - 3600],
            |r| r.get(0),
        )
        .map_err(err)?;
    if recent >= PER_HOUR {
        return Ok(false);
    }
    let detail = detail.map(|d| d.chars().take(64).collect::<String>());
    conn.execute(
        "INSERT INTO visits (visitor, at, kind, detail) VALUES (?1, ?2, ?3, ?4)",
        params![visitor, at, kind, detail],
    )
    .map_err(err)?;
    Ok(true)
}

#[derive(Default)]
struct Visitor {
    days: HashSet<i64>,
    kinds: HashMap<String, u32>,
}

/// The measures of the friends test, fixed before it starts (roadmap, decision 7): how many
/// visitors came back the next day and after a week, and what they did. Days are UTC days.
pub fn summary(conn: &Connection, now: i64) -> Result<Value, String> {
    let mut stmt = conn
        .prepare("SELECT visitor, at, kind FROM visits")
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(err)?;
    let mut visitors: HashMap<String, Visitor> = HashMap::new();
    let mut per_day: BTreeMap<i64, (HashSet<String>, u32)> = BTreeMap::new();
    for row in rows {
        let (id, at, kind) = row.map_err(err)?;
        let v = visitors.entry(id.clone()).or_default();
        v.days.insert(at / DAY);
        *v.kinds.entry(kind.clone()).or_default() += 1;
        let d = per_day.entry(at / DAY).or_default();
        d.0.insert(id);
        if kind == "prediction" {
            d.1 += 1;
        }
    }
    let today = now / DAY;
    let (mut d1_base, mut d1, mut d7_base, mut d7) = (0, 0, 0, 0);
    let mut firsts: BTreeMap<i64, u32> = BTreeMap::new();
    for v in visitors.values() {
        let first = *v.days.iter().min().expect("a visitor has a day");
        *firsts.entry(first).or_default() += 1;
        if first < today {
            d1_base += 1;
            d1 += u32::from(v.days.contains(&(first + 1)));
        }
        if first + 7 <= today {
            d7_base += 1;
            d7 += u32::from(v.days.iter().any(|&d| d >= first + 7));
        }
    }
    let n = visitors.len() as u32;
    let did = |kind: &str| {
        visitors
            .values()
            .filter(|v| v.kinds.contains_key(kind))
            .count() as u32
    };
    let total = |kind: &str| {
        visitors
            .values()
            .map(|v| v.kinds.get(kind).copied().unwrap_or(0))
            .sum::<u32>()
    };
    let share =
        |part: u32, of: u32| (of > 0).then(|| (part as f64 * 1000.0 / of as f64).round() / 10.0);
    let days: Vec<Value> = per_day
        .iter()
        .map(|(day, (ids, predictions))| {
            json!({
                "day": day * DAY,
                "visitors": ids.len(),
                "new": firsts.get(day).copied().unwrap_or(0),
                "predictions": predictions,
            })
        })
        .collect();
    Ok(json!({
        "visitors": n,
        "returned_day_1": { "of": d1_base, "returned": d1, "pct": share(d1, d1_base) },
        "returned_day_7": { "of": d7_base, "returned": d7, "pct": share(d7, d7_base) },
        "opened_a_story_pct": share(did("story"), n),
        "opened_a_card_pct": share(did("card"), n),
        "made_a_prediction_pct": share(did("prediction"), n),
        "replayed_pct": share(did("replay"), n),
        "predictions": total("prediction"),
        "days": days,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0123456789abcdef";
    const B: &str = "fedcba9876543210";

    #[test]
    fn counts_returns_and_actions() {
        let conn = open(Path::new(":memory:")).unwrap();
        let day0 = 20_000 * DAY;
        assert!(record(&conn, A, "visit", None, day0 + 10).unwrap());
        assert!(record(&conn, A, "story", Some("comeback"), day0 + 20).unwrap());
        assert!(record(&conn, A, "visit", None, day0 + DAY + 5).unwrap());
        assert!(record(&conn, A, "visit", None, day0 + 8 * DAY).unwrap());
        assert!(record(&conn, B, "visit", None, day0 + 30).unwrap());
        assert!(record(&conn, B, "prediction", Some("survive"), day0 + 40).unwrap());
        // Unknown kinds and malformed ids are dropped.
        assert!(!record(&conn, B, "hack", None, day0).unwrap());
        assert!(!record(&conn, "x'; DROP", "visit", None, day0).unwrap());

        let s = summary(&conn, day0 + 9 * DAY).unwrap();
        assert_eq!(s["visitors"], 2);
        assert_eq!(s["returned_day_1"]["returned"], 1);
        assert_eq!(s["returned_day_1"]["pct"], 50.0);
        assert_eq!(s["returned_day_7"]["returned"], 1);
        assert_eq!(s["opened_a_story_pct"], 50.0);
        assert_eq!(s["predictions"], 1);
        assert_eq!(s["days"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn a_flood_from_one_visitor_is_capped() {
        let conn = open(Path::new(":memory:")).unwrap();
        let kept = (0..PER_HOUR + 50)
            .filter(|&i| record(&conn, A, "card", None, 1_000_000 + i).unwrap())
            .count();
        assert_eq!(kept as i64, PER_HOUR);
    }
}
