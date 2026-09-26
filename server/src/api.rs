//! The read API v0 (spec §23). Reads come from the live world or from SQLite; nothing here can
//! change the world. The one write is anonymous visit counts for the early tests, kept apart from
//! the world's data (`visits`).

use std::sync::Arc;

use axum::extract::{Path, Query, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use protogaea_core::run::hex;
use protogaea_core::World;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::model::{archetype, Roots};
use crate::store;
use crate::visits;
use crate::world::{load_world, Shared};

type Api = Arc<Shared>;

pub struct ApiError(StatusCode, &'static str, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1, "message": self.2 }))).into_response()
    }
}

fn not_found(what: String) -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "E_NOT_FOUND", what)
}

fn internal(message: String) -> ApiError {
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, "E_INTERNAL", message)
}

type Reply = Result<Json<Value>, ApiError>;

/// Runs a database read off the async runtime.
async fn read<T, F>(api: &Api, f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&rusqlite::Connection) -> Result<T, String> + Send + 'static,
{
    let path = api.db();
    tokio::task::spawn_blocking(move || {
        let conn = store::reader(&path)?;
        f(&conn)
    })
    .await
    .map_err(|e| internal(e.to_string()))?
    .map_err(internal)
}

/// With a viewer, `/` leads to it; without one, to a page listing the API.
pub fn router(api: Api, viewer: bool) -> Router {
    let home = if viewer {
        get(|| async { Redirect::temporary("/app/") })
    } else {
        get(index)
    };
    Router::new()
        .route("/", home)
        .route("/v0/world", get(world))
        .route("/v0/ruleset", get(ruleset))
        .route("/v0/map", get(map))
        .route("/v0/rifts", get(rifts))
        .route("/v0/epochs", get(epochs))
        .route("/v0/epochs/{n}", get(epoch))
        .route("/v0/snapshots", get(snapshots))
        .route("/v0/snapshots/{epoch}", get(snapshot))
        .route("/v0/clades", get(clades))
        .route("/v0/clades/{id}", get(clade))
        .route("/v0/organisms/{id}", get(organism))
        .route("/v0/museum", get(museum))
        .route("/v0/events", get(events))
        .route("/v0/muller", get(muller))
        .route("/v0/tree", get(tree))
        .route("/v0/stories", get(stories))
        .route("/v0/digest", get(digest))
        .route("/v0/replay", get(replay))
        .route("/v0/visits", post(visit))
        .route("/v0/visits/summary", get(visit_summary))
        .route("/v0/proofs/{epoch}/organism/{id}", get(proof))
        .route("/health", get(|| async { "ok" }))
        .with_state(api)
}

/// Puts everything behind HTTP Basic authentication except `/health`, which stays open for
/// monitoring. Applied last, so it covers the viewer's files too.
pub fn protect(app: Router, credentials: Option<String>) -> Router {
    match credentials {
        Some(expected) => app.layer(axum::middleware::from_fn(move |req, next| {
            let expected = expected.clone();
            async move { basic_auth(req, next, expected).await }
        })),
        None => app,
    }
}

async fn basic_auth(req: Request, next: Next, expected: String) -> Response {
    let ok = req.uri().path() == "/health"
        || req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v == expected);
    if ok {
        next.run(req).await
    } else {
        let mut res = (StatusCode::UNAUTHORIZED, "authorization required").into_response();
        res.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"Protogaea\", charset=\"UTF-8\""),
        );
        res
    }
}

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn world(State(api): State<Api>) -> Reply {
    let live = api.live.read().expect("the lock is never poisoned");
    let w = &live.world;
    Ok(Json(json!({
        "world_id": hex(&w.world_id),
        "ruleset_id": hex(&w.ruleset_id),
        "seed": api.seed,
        "width": w.width,
        "height": w.height,
        "epochs_per_day": api.rules.epochs_per_day,
        "season_days": api.rules.season_days,
        "epoch_seconds": api.epoch_seconds,
        "next_epoch_ms": live.next_epoch_ms,
        "archive_every": api.archive_every,
        "finished": w.finished(&api.rules),
        "header": live.header,
    })))
}

async fn ruleset(State(api): State<Api>) -> Reply {
    Ok(Json(
        serde_json::to_value(&api.rules).map_err(|e| internal(e.to_string()))?,
    ))
}

#[derive(Deserialize)]
struct MapQuery {
    epoch: Option<u64>,
}

/// A state in a compact form for the map, one array per field: the latest one, or an archived
/// snapshot's with `?epoch=`.
async fn map(State(api): State<Api>, Query(q): Query<MapQuery>) -> Reply {
    // The lock is released before any await.
    let latest = {
        let live = api.live.read().expect("the lock is never poisoned");
        q.epoch
            .is_none_or(|e| e == live.world.epoch)
            .then(|| live.world.clone())
    };
    if let Some(world) = latest {
        let names = living_names(&api, &world).await?;
        return Ok(Json(map_json(&world, &names)));
    }
    let epoch = q.epoch.unwrap_or_default();
    let path = api.archived(epoch);
    if !path.exists() {
        return Err(no_snapshot(&api, epoch));
    }
    let world = tokio::task::spawn_blocking(move || load_world(&path))
        .await
        .map_err(|e| internal(e.to_string()))?
        .map_err(internal)?;
    let names = living_names(&api, &world).await?;
    Ok(Json(map_json(&world, &names)))
}

/// The stored names of the world's living named clades.
async fn living_names(
    api: &Api,
    world: &World,
) -> Result<serde_json::Map<String, Value>, ApiError> {
    let threshold = api.rules.clade_name_threshold;
    let ids: Vec<u32> = world
        .clades
        .values()
        .filter(|c| c.peak_living >= threshold)
        .map(|c| c.id)
        .collect();
    let names = read(api, move |c| store::clade_names(c, &ids)).await?;
    Ok(names
        .into_iter()
        .map(|(id, n)| (id.to_string(), Value::String(n)))
        .collect())
}

fn no_snapshot(api: &Api, epoch: u64) -> ApiError {
    ApiError(
        StatusCode::NOT_FOUND,
        "E_NO_SNAPSHOT",
        format!(
            "no snapshot at epoch {epoch}; snapshots are kept every {} epochs",
            api.archive_every
        ),
    )
}

fn map_json(w: &World, names: &serde_json::Map<String, Value>) -> Value {
    let n = w.organisms.len();
    let mut traits = Vec::with_capacity(n * 6);
    let (mut id, mut cell, mut clade, mut hue, mut kind, mut energy, mut age) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    for o in &w.organisms {
        id.push(o.id);
        cell.push(o.cell);
        clade.push(o.clade_id);
        hue.push(o.genome.hue);
        kind.push(archetype(&o.genome.traits));
        energy.push(o.energy);
        age.push(o.age);
        traits.extend_from_slice(&o.genome.traits);
    }
    json!({
        "epoch": w.epoch,
        "width": w.width,
        "height": w.height,
        "biome": w.cells.iter().map(|c| c.biome as u8).collect::<Vec<_>>(),
        "food": w.cells.iter().map(|c| c.food).collect::<Vec<_>>(),
        "moisture": w.cells.iter().map(|c| c.moisture).collect::<Vec<_>>(),
        "rift": w.cells.iter().map(|c| c.rift as u8).collect::<Vec<_>>(),
        "effects": w.effects,
        // Names of the living clades that have reached the naming threshold.
        "names": names,
        "organisms": {
            "id": id, "cell": cell, "clade": clade, "hue": hue,
            "kind": kind, "energy": energy, "age": age,
            // Six traits per organism, in the order M P G H D F.
            "traits": traits,
        },
    })
}

/// The rift schedule, fixed at genesis (spec §4).
async fn rifts(State(api): State<Api>) -> Reply {
    let live = api.live.read().expect("the lock is never poisoned");
    Ok(Json(
        json!({ "rifts": live.world.rifts, "schedule": api.rules.rifts }),
    ))
}

#[derive(Deserialize)]
struct Range {
    from: Option<u64>,
    to: Option<u64>,
    step: Option<u64>,
    limit: Option<u32>,
}

async fn epochs(State(api): State<Api>, Query(q): Query<Range>) -> Reply {
    let latest = api
        .live
        .read()
        .expect("the lock is never poisoned")
        .world
        .epoch;
    let (from, to) = (q.from.unwrap_or(0), q.to.unwrap_or(latest));
    let (step, limit) = (q.step.unwrap_or(1), q.limit.unwrap_or(2000).min(5000));
    let rows = read(&api, move |c| store::headers(c, from, to, step, limit)).await?;
    Ok(Json(json!({ "epochs": rows })))
}

async fn epoch(State(api): State<Api>, Path(n): Path<u64>) -> Reply {
    read(&api, move |c| store::header(c, n))
        .await?
        .map(Json)
        .ok_or_else(|| not_found(format!("no epoch {n}")))
}

async fn snapshots(State(api): State<Api>) -> Reply {
    let dir = api.data.join("snapshots");
    let mut epochs: Vec<u64> = std::fs::read_dir(&dir)
        .map_err(|e| internal(e.to_string()))?
        .filter_map(|e| {
            e.ok()?
                .file_name()
                .to_str()?
                .strip_suffix(".json")?
                .parse()
                .ok()
        })
        .collect();
    epochs.sort_unstable();
    Ok(Json(
        json!({ "every": api.archive_every, "epochs": epochs }),
    ))
}

async fn snapshot(State(api): State<Api>, Path(epoch): Path<u64>) -> Result<Response, ApiError> {
    let path = api.archived(epoch);
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| not_found(format!("no snapshot at epoch {epoch}")))?;
    Ok(([(header::CONTENT_TYPE, "application/json")], bytes).into_response())
}

#[derive(Deserialize)]
struct CladeQuery {
    living: Option<bool>,
    named: Option<bool>,
    limit: Option<u32>,
}

async fn clades(State(api): State<Api>, Query(q): Query<CladeQuery>) -> Reply {
    let min_peak = if q.named.unwrap_or(false) {
        api.rules.clade_name_threshold
    } else {
        0
    };
    let (living, limit) = (
        q.living.unwrap_or(false),
        q.limit.unwrap_or(1000).min(10000),
    );
    let rows = read(&api, move |c| store::clades(c, living, min_peak, limit)).await?;
    Ok(Json(json!({ "clades": rows })))
}

async fn clade(State(api): State<Api>, Path(id): Path<u32>) -> Reply {
    let (clade, parent) = read(&api, move |c| {
        let clade = store::clade(c, id)?;
        let parent = match clade.as_ref().and_then(|v| v["parent_id"].as_u64()) {
            Some(p) if p > 0 => store::clade_names(c, &[p as u32])?,
            _ => Vec::new(),
        };
        Ok((clade, parent))
    })
    .await?;
    let mut clade = clade.ok_or_else(|| not_found(format!("no clade {id}")))?;
    clade["parent_name"] = parent
        .into_iter()
        .next()
        .map_or(Value::Null, |(_, n)| Value::String(n));
    Ok(Json(clade))
}

/// Which shown clade each clade belongs to: itself if it is named or a founder, otherwise its
/// nearest named ancestor. Thousands of clades split off in a season and most stay tiny; folding
/// them keeps the tree and the Muller plot about lineages.
fn fold(rows: &[store::TreeRow], threshold: u32) -> std::collections::HashMap<u32, u32> {
    use std::collections::HashMap;
    let info: HashMap<u32, (u32, bool)> = rows
        .iter()
        .map(|r| (r.0, (r.1, r.4 >= threshold || r.1 == 0)))
        .collect();
    let mut shown: HashMap<u32, u32> = HashMap::with_capacity(rows.len());
    // Rows come in id order, and a parent always has a smaller id than its children.
    for r in rows {
        let (id, parent) = (r.0, r.1);
        let (_, named) = info[&id];
        let target = if named {
            id
        } else {
            shown.get(&parent).copied().unwrap_or(id)
        };
        shown.insert(id, target);
    }
    shown
}

/// The named clades of the season (and the founders), each with its nearest named ancestor as
/// parent: the clade tree and the Muller plot are drawn from it.
async fn tree(State(api): State<Api>) -> Reply {
    let rows = read(&api, store::tree).await?;
    let threshold = api.rules.clade_name_threshold;
    let shown = fold(&rows, threshold);
    let clades: Vec<Value> = rows
        .iter()
        .filter(|(id, ..)| shown.get(id) == Some(id))
        .map(|(id, parent, founded, extinct, peak, reference, name)| {
            let genome: Option<protogaea_core::Genome> = serde_json::from_str(reference).ok();
            let hue = genome.map_or(0, |g| g.hue);
            let parent = if *parent == 0 {
                0
            } else {
                shown.get(parent).copied().unwrap_or(0)
            };
            json!([id, parent, founded, extinct, peak, hue, name])
        })
        .collect();
    Ok(Json(json!({
        "fields": ["id", "parent", "founded", "extinct", "peak", "hue", "name"],
        "name_threshold": threshold,
        "clades": clades,
    })))
}

async fn organism(State(api): State<Api>, Path(id): Path<u64>) -> Reply {
    let mut o = read(&api, move |c| store::organism(c, id))
        .await?
        .ok_or_else(|| not_found(format!("no organism {id}")))?;
    let live = api.live.read().expect("the lock is never poisoned");
    if let Ok(i) = live.world.organisms.binary_search_by_key(&id, |o| o.id) {
        let x = &live.world.organisms[i];
        o["living"] =
            json!({ "epoch": live.world.epoch, "cell": x.cell, "age": x.age, "energy": x.energy });
    }
    Ok(Json(o))
}

#[derive(Deserialize)]
struct Limit {
    limit: Option<u32>,
}

async fn museum(State(api): State<Api>, Query(q): Query<Limit>) -> Reply {
    let min_peak = api.rules.clade_name_threshold;
    let limit = q.limit.unwrap_or(1000).min(10000);
    let rows = read(&api, move |c| store::museum(c, min_peak, limit)).await?;
    Ok(Json(json!({ "museum": rows })))
}

#[derive(Deserialize)]
struct EventQuery {
    /// The newest events first, older than this id (`before=0` for the latest).
    before: Option<i64>,
    cursor: Option<i64>,
    limit: Option<u32>,
    kind: Option<String>,
    clade: Option<u32>,
}

/// Events after a cursor, oldest first: the cursor is the id of the last event seen and `next`
/// continues. With `before`, the newest events first instead, for a feed. `names` gives the
/// names of the named clades the events mention.
async fn events(State(api): State<Api>, Query(q): Query<EventQuery>) -> Reply {
    let (cursor, limit) = (q.cursor.unwrap_or(0), q.limit.unwrap_or(200).min(1000));
    let newest_first = q.before.is_some();
    let (rows, refs) = read(&api, move |c| {
        let rows = match q.before {
            Some(before) => store::events_before(c, before, limit, q.kind.as_deref(), q.clade)?,
            None => store::events(c, cursor, limit, q.kind.as_deref(), q.clade)?,
        };
        let mut ids: Vec<u32> = rows
            .iter()
            .flat_map(|e| {
                [
                    e["clade_id"].as_u64(),
                    e["data"]["parent_id"].as_u64(),
                    e["data"]["from"].as_u64(),
                ]
            })
            .flatten()
            .filter(|&id| id > 0)
            .map(|id| id as u32)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let refs = store::clade_names(c, &ids)?;
        Ok((rows, refs))
    })
    .await?;
    let names: serde_json::Map<String, Value> = refs
        .into_iter()
        .map(|(id, n)| (id.to_string(), Value::String(n)))
        .collect();
    let fallback = if newest_first { 0 } else { cursor };
    let next = rows
        .last()
        .and_then(|e| e["id"].as_i64())
        .unwrap_or(fallback);
    Ok(Json(
        json!({ "events": rows, "next": next, "names": names }),
    ))
}

#[derive(Deserialize)]
struct StoryQuery {
    /// From this epoch on; by default, the last world day.
    since: Option<u64>,
    limit: Option<u32>,
}

/// The stories the detectors found (roadmap B4), the most important first, with the names of
/// the clades they are about.
async fn stories(State(api): State<Api>, Query(q): Query<StoryQuery>) -> Reply {
    let latest = api
        .live
        .read()
        .expect("the lock is never poisoned")
        .world
        .epoch;
    let since = q
        .since
        .unwrap_or_else(|| latest.saturating_sub(u64::from(api.rules.epochs_per_day)));
    let limit = q.limit.unwrap_or(20).min(200);
    let (rows, names) = read(&api, move |c| {
        // A varied selection: the best story of each clade and kind, then the rest by score.
        let all = store::stories(c, since, 500)?;
        let (mut first, mut rest) = (Vec::new(), Vec::new());
        let mut clades = std::collections::HashSet::new();
        let mut kinds = std::collections::HashSet::new();
        for s in all {
            let clade = s["clade_id"].as_i64();
            let kind = s["kind"].as_str().unwrap_or_default().to_string();
            if clade.is_none_or(|c| !clades.contains(&c)) && !kinds.contains(&kind) {
                if let Some(c) = clade {
                    clades.insert(c);
                }
                kinds.insert(kind);
                first.push(s);
            } else {
                rest.push(s);
            }
        }
        let rows: Vec<Value> = first.into_iter().chain(rest).take(limit as usize).collect();
        let mut ids: Vec<u32> = rows
            .iter()
            .flat_map(|s| [s["clade_id"].as_u64(), s["other_id"].as_u64()])
            .flatten()
            .map(|id| id as u32)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let names = store::clade_names(c, &ids)?;
        Ok((rows, names))
    })
    .await?;
    let names: serde_json::Map<String, Value> = names
        .into_iter()
        .map(|(id, n)| (id.to_string(), Value::String(n)))
        .collect();
    Ok(Json(
        json!({ "since": since, "stories": rows, "names": names }),
    ))
}

#[derive(Deserialize)]
struct DigestQuery {
    since: u64,
}

/// "While you were away" (spec §7): what changed since an epoch the viewer last saw.
async fn digest(State(api): State<Api>, Query(q): Query<DigestQuery>) -> Reply {
    let (now, header) = {
        let live = api.live.read().expect("the lock is never poisoned");
        (
            live.world.epoch,
            serde_json::to_value(&live.header).unwrap_or(Value::Null),
        )
    };
    let since = q.since.min(now);
    let (then, counts, bridges, stories, names) = read(&api, move |c| {
        let then = store::header(c, since)?;
        let counts = store::event_counts(c, since)?;
        let bridges = store::events_since(c, since, "bridge_closed")?;
        // The best stories of the time away, one per clade and kind first.
        let all = store::stories(c, since + 1, 300)?;
        let (mut first, mut rest) = (Vec::new(), Vec::new());
        let mut seen = std::collections::HashSet::new();
        for s in all {
            let key = (
                s["clade_id"].as_i64(),
                s["kind"].as_str().unwrap_or_default().to_string(),
            );
            if seen.insert(key) {
                first.push(s);
            } else {
                rest.push(s);
            }
        }
        let stories: Vec<Value> = first.into_iter().chain(rest).take(6).collect();
        let mut ids: Vec<u32> = stories
            .iter()
            .flat_map(|s| [s["clade_id"].as_u64(), s["other_id"].as_u64()])
            .flatten()
            .map(|id| id as u32)
            .collect();
        for h in [then.as_ref(), None].into_iter().flatten() {
            if let Some(d) = h["dominant_clade"].as_u64() {
                ids.push(d as u32);
            }
        }
        ids.sort_unstable();
        ids.dedup();
        let names = store::clade_names(c, &ids)?;
        Ok((then, counts, bridges, stories, names))
    })
    .await?;
    let mut names: serde_json::Map<String, Value> = names
        .into_iter()
        .map(|(id, n)| (id.to_string(), Value::String(n)))
        .collect();
    if let Some(d) = header["dominant_clade"].as_u64() {
        let d = d as u32;
        if !names.contains_key(&d.to_string()) {
            let found = read(&api, move |c| store::clade_names(c, &[d])).await?;
            for (id, n) in found {
                names.insert(id.to_string(), Value::String(n));
            }
        }
    }
    Ok(Json(json!({
        "since": since,
        "now": now,
        "then": then,
        "header": header,
        "counts": counts.into_iter().collect::<std::collections::BTreeMap<_, _>>(),
        "bridges_closed": bridges,
        "stories": stories,
        "names": names,
    })))
}

#[derive(Deserialize)]
struct ReplayQuery {
    from: Option<u64>,
    step: Option<u64>,
}

/// Frames of the recent past for the replay, binary: for each frame the epoch (u32) and the
/// number of organisms (u32), then per organism the low 32 bits of its id (u32), its cell (u16),
/// clade (u32), hue (u16) and archetype (u8); all little-endian.
async fn replay(
    State(api): State<Api>,
    Query(q): Query<ReplayQuery>,
) -> Result<Response, ApiError> {
    let now = api
        .live
        .read()
        .expect("the lock is never poisoned")
        .world
        .epoch;
    let from = q
        .from
        .unwrap_or_else(|| now.saturating_sub(u64::from(api.rules.epochs_per_day)));
    let step = q.step.unwrap_or(2).clamp(1, 48);
    let frames = read(&api, move |c| store::frames(c, from, now, step)).await?;
    let mut out = Vec::new();
    for (epoch, data) in frames {
        out.extend_from_slice(&(epoch as u32).to_le_bytes());
        out.extend_from_slice(&((data.len() / store::FRAME_ORGANISM) as u32).to_le_bytes());
        out.extend_from_slice(&data);
    }
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], out).into_response())
}

#[derive(Deserialize)]
struct Visit {
    visitor: String,
    kind: String,
    detail: Option<String>,
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// A visitor's action, from the viewer. Unknown kinds and malformed ids are dropped quietly.
async fn visit(State(api): State<Api>, Json(v): Json<Visit>) -> Result<StatusCode, ApiError> {
    let path = api.data.join(visits::DB);
    tokio::task::spawn_blocking(move || {
        let conn = visits::open(&path)?;
        visits::record(&conn, &v.visitor, &v.kind, v.detail.as_deref(), unix_now())
    })
    .await
    .map_err(|e| internal(e.to_string()))?
    .map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// The measures of the friends test: returns on day 1 and day 7, what visitors did, visitors by day.
async fn visit_summary(State(api): State<Api>) -> Reply {
    let path = api.data.join(visits::DB);
    let summary = tokio::task::spawn_blocking(move || {
        let conn = visits::open(&path)?;
        visits::summary(&conn, unix_now())
    })
    .await
    .map_err(|e| internal(e.to_string()))?
    .map_err(internal)?;
    Ok(Json(summary))
}

#[derive(Deserialize)]
struct MullerQuery {
    from: Option<u64>,
    step: Option<u64>,
}

/// Clade counts over time: rows of `[epoch, clade, living]`, one sample per world hour
/// (or per `step` hours).
async fn muller(State(api): State<Api>, Query(q): Query<MullerQuery>) -> Reply {
    let (from, step) = (q.from.unwrap_or(0), q.step.unwrap_or(1));
    let threshold = api.rules.clade_name_threshold;
    let rows = read(&api, move |c| {
        let rows = store::muller(c, from, step)?;
        let shown = fold(&store::tree(c)?, threshold);
        // Unnamed clades count toward their nearest named ancestor.
        let mut folded: std::collections::BTreeMap<(i64, i64), i64> =
            std::collections::BTreeMap::new();
        for [epoch, clade, living] in rows {
            let to = shown.get(&(clade as u32)).map_or(clade, |&s| i64::from(s));
            *folded.entry((epoch, to)).or_default() += living;
        }
        Ok(folded
            .into_iter()
            .map(|((e, c), n)| [e, c, n])
            .collect::<Vec<_>>())
    })
    .await?;
    Ok(Json(
        json!({ "every": store::MULLER_EVERY * step.max(1), "rows": rows }),
    ))
}

/// An organism's inclusion proof against the `state_root` of an epoch (spec §15): for the
/// latest epoch, or for an epoch with an archived snapshot.
async fn proof(State(api): State<Api>, Path((epoch, id)): Path<(u64, u64)>) -> Reply {
    let latest = {
        let live = api.live.read().expect("the lock is never poisoned");
        (live.world.epoch == epoch).then(|| {
            live.world
                .prove_organism(id)
                .map(|p| (p, live.world.state_root()))
        })
    };
    let found = match latest {
        Some(found) => found,
        None => {
            let path = api.archived(epoch);
            if !path.exists() {
                return Err(no_snapshot(&api, epoch));
            }
            let world = tokio::task::spawn_blocking(move || load_world(&path))
                .await
                .map_err(|e| internal(e.to_string()))?
                .map_err(internal)?;
            world.prove_organism(id).map(|p| (p, world.state_root()))
        }
    };
    let (p, root) =
        found.ok_or_else(|| not_found(format!("organism {id} is not alive at epoch {epoch}")))?;
    Ok(Json(json!({
        "epoch": epoch,
        "state_root": hex(&root),
        "verified": p.verify(&root),
        "organism": p.organism,
        "index": p.index,
        "size": p.size,
        "path": p.path.iter().map(|h| hex(h)).collect::<Vec<_>>(),
        "roots": Roots::from(&p.roots),
    })))
}
