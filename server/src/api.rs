//! The read API v0 (spec §23). Reads come from the live world or from SQLite; nothing here can
//! change the world.

use std::sync::Arc;

use axum::extract::{Path, Query, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Json, Router};
use protogaea_core::run::hex;
use protogaea_core::World;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::model::{archetype, Roots};
use crate::store;
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
    {
        let live = api.live.read().expect("the lock is never poisoned");
        if q.epoch.is_none_or(|e| e == live.world.epoch) {
            return Ok(Json(map_json(&live.world)));
        }
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
    Ok(Json(map_json(&world)))
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

fn map_json(w: &World) -> Value {
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
    read(&api, move |c| store::clade(c, id))
        .await?
        .map(Json)
        .ok_or_else(|| not_found(format!("no clade {id}")))
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
/// continues. With `before`, the newest events first instead, for a feed.
async fn events(State(api): State<Api>, Query(q): Query<EventQuery>) -> Reply {
    let (cursor, limit) = (q.cursor.unwrap_or(0), q.limit.unwrap_or(200).min(1000));
    if let Some(before) = q.before {
        let rows = read(&api, move |c| {
            store::events_before(c, before, limit, q.kind.as_deref(), q.clade)
        })
        .await?;
        let next = rows.last().and_then(|e| e["id"].as_i64()).unwrap_or(0);
        return Ok(Json(json!({ "events": rows, "next": next })));
    }
    let rows = read(&api, move |c| {
        store::events(c, cursor, limit, q.kind.as_deref(), q.clade)
    })
    .await?;
    let next = rows.last().and_then(|e| e["id"].as_i64()).unwrap_or(cursor);
    Ok(Json(json!({ "events": rows, "next": next })))
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
    let rows = read(&api, move |c| store::muller(c, from, step)).await?;
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
