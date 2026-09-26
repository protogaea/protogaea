//! The HTTP side of wishes and sparks (protocol §4, §5, §6): the open window, wishes with their
//! first spark, spark batches, tree heads and log proofs.
//!
//! Intake order for a spark (spec §18): size and format, the window, the wish, duplicates, rate
//! limits, then one PoW check, the log and the receipt. The PoW is checked outside the intake lock
//! with the reference C yespower, on at most two threads; a full queue answers `E_OVERLOADED`. An
//! invalid PoW bans the key and the address for an hour: an honest client never sends one.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use protogaea_core::World;
use protogaea_pow::{meets, spark_input, weight, SPARK};
use protogaea_protocol::spark::{parse_batch, Spark};
use protogaea_protocol::wish::{Action, Source, Wish};
use protogaea_protocol::{hex, Hash};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::intake::{receipt_json, sth_json, Refusal, Ticket};
use crate::world::Shared;

type Api = std::sync::Arc<Shared>;

const BAN: Duration = Duration::from_secs(3600);
/// Spark batches per address: a bucket of this many, refilled at this rate per second.
const BUCKET: f64 = 40.0;
const REFILL: f64 = 10.0;
/// Sparks waiting for a PoW check at most, and threads checking them.
const QUEUE: usize = 512;
static QUEUED: AtomicUsize = AtomicUsize::new(0);
static CHECKERS: LazyLock<tokio::sync::Semaphore> =
    LazyLock::new(|| tokio::sync::Semaphore::new(2));

struct Guard {
    buckets: HashMap<IpAddr, (f64, Instant)>,
    banned: HashMap<Vec<u8>, Instant>,
}

static GUARD: LazyLock<Mutex<Guard>> = LazyLock::new(|| {
    Mutex::new(Guard {
        buckets: HashMap::new(),
        banned: HashMap::new(),
    })
});

fn ip_key(ip: IpAddr) -> Vec<u8> {
    match ip {
        IpAddr::V4(a) => a.octets().to_vec(),
        IpAddr::V6(a) => a.octets().to_vec(),
    }
}

impl Guard {
    fn banned(&mut self, key: &[u8]) -> bool {
        match self.banned.get(key) {
            Some(until) if *until > Instant::now() => true,
            Some(_) => {
                self.banned.remove(key);
                false
            }
            None => false,
        }
    }

    fn take(&mut self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let (tokens, last) = self.buckets.entry(ip).or_insert((BUCKET, now));
        *tokens = (*tokens + now.duration_since(*last).as_secs_f64() * REFILL).min(BUCKET);
        *last = now;
        if self.buckets.len() > 100_000 {
            self.buckets
                .retain(|_, (_, at)| now.duration_since(*at) < Duration::from_secs(60));
        }
        let (tokens, _) = self.buckets.get_mut(&ip).expect("just inserted");
        if *tokens >= 1.0 {
            *tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

pub fn routes() -> Router<Api> {
    Router::new()
        .route("/v0/operator", get(operator))
        .route("/v0/window", get(window))
        .route("/v0/proposals", post(propose).get(proposals))
        .route("/v0/sparks", post(sparks))
        .route("/v0/sth", get(sth))
        .route("/v0/ledger", get(ledger))
        .route("/v0/miracles", get(miracles))
        .route("/v0/log/{epoch}/inclusion", get(inclusion))
        .route("/v0/log/{epoch}/consistency", get(consistency))
}

fn refuse(r: Refusal) -> Response {
    let status = match r {
        Refusal::Limit => StatusCode::TOO_MANY_REQUESTS,
        Refusal::UnknownProposal => StatusCode::NOT_FOUND,
        Refusal::Duplicate | Refusal::ProposalClosed | Refusal::WindowClosed => {
            StatusCode::CONFLICT
        }
        _ => StatusCode::BAD_REQUEST,
    };
    let message = match r {
        Refusal::ActionInvalid(why) => why,
        _ => "",
    };
    (
        status,
        Json(json!({ "error": r.code(), "message": message })),
    )
        .into_response()
}

fn internal(e: String) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "E_INTERNAL", "message": e })),
    )
        .into_response()
}

fn overloaded() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::RETRY_AFTER, "5")],
        Json(json!({ "error": "E_OVERLOADED", "message": "" })),
    )
        .into_response()
}

async fn operator(State(api): State<Api>) -> Json<Value> {
    let intake = api.intake.lock().expect("the lock is never poisoned");
    Json(json!({ "operator": hex(&intake.operator) }))
}

async fn window(State(api): State<Api>) -> Json<Value> {
    let closes = api
        .live
        .read()
        .expect("the lock is never poisoned")
        .next_epoch_ms;
    let intake = api.intake.lock().expect("the lock is never poisoned");
    Json(json!({
        "epoch": intake.epoch,
        "open": intake.open,
        "challenge": hex(&intake.challenge),
        // u64 as a string: JavaScript numbers lose precision beyond 2^53.
        "target": intake.target.to_string(),
        "weight": weight(intake.target).to_string(),
        "world_id": hex(&intake.world_id),
        "ruleset_id": hex(&intake.ruleset_id),
        "closes_at_ms": closes,
        "sth": intake.sth.as_ref().map(sth_json),
    }))
}

/// The PoW of a spark for a ticket, with the reference C yespower kept per thread.
fn pow(t: &Ticket, s: &Spark) -> Option<u64> {
    thread_local! {
        static HASHER: std::cell::RefCell<Option<protogaea_pow::c::Hasher>> = const { std::cell::RefCell::new(None) };
    }
    let input = spark_input(
        &t.world_id,
        t.epoch,
        &t.challenge,
        &s.proposal_id,
        &s.miner,
        s.nonce,
    );
    let hash = HASHER.with(|h| {
        h.borrow_mut()
            .get_or_insert_with(|| protogaea_pow::c::Hasher::new(SPARK))
            .hash(&input)
    });
    meets(&hash, t.target).then(|| weight(t.target))
}

/// What the PoW check found: a spark with its weight, a spark for the window that just closed,
/// or no spark at all.
#[derive(Clone, Copy)]
enum Pow {
    Valid(u64),
    Late,
    Invalid,
}

/// Checks the PoW of sparks on the checking threads. `None` when the queue is full. A spark that
/// fails the open window is checked against the one before, so a client that has not yet seen
/// the window change is told it is late instead of being banned.
async fn check_pow(t: Ticket, previous: Option<Ticket>, sparks: Vec<Spark>) -> Option<Vec<Pow>> {
    let n = sparks.len();
    if QUEUED.fetch_add(n, Ordering::SeqCst) + n > QUEUE {
        QUEUED.fetch_sub(n, Ordering::SeqCst);
        return None;
    }
    let permit = CHECKERS.acquire().await.expect("the semaphore stays open");
    let out = tokio::task::spawn_blocking(move || {
        sparks
            .iter()
            .map(|s| match pow(&t, s) {
                Some(w) => Pow::Valid(w),
                None if previous.is_some_and(|p| pow(&p, s).is_some()) => Pow::Late,
                None => Pow::Invalid,
            })
            .collect::<Vec<_>>()
    })
    .await
    .ok();
    drop(permit);
    QUEUED.fetch_sub(n, Ordering::SeqCst);
    out
}

/// Whether a wish can apply to the latest published state (spec §17, the soft check).
fn soft_check(world: &World, w: &Wish) -> Result<(), &'static str> {
    let inside = |(x, y): (u8, u8)| u16::from(x) < world.width && u16::from(y) < world.height;
    match &w.action {
        Action::Weather { x, y, .. } => inside((*x, *y))
            .then_some(())
            .ok_or("the area is outside the map"),
        Action::Migrate { clade_id, from, to } => {
            if !inside(*from) || !inside(*to) {
                return Err("the area is outside the map");
            }
            match world.clades.get(clade_id) {
                Some(c) if c.living > 0 => Ok(()),
                _ => Err("no such living clade"),
            }
        }
        Action::Revive {
            source,
            entry_id,
            steps,
            at,
        } => {
            if !inside(*at) {
                return Err("the start is outside the map");
            }
            if steps.iter().any(|(i, j)| *i >= 6 || *j >= 6 || i == j) {
                return Err("a mutation step names no two traits");
            }
            let known = match source {
                Source::Museum => world.museum.iter().any(|m| m.clade_id == *entry_id),
                Source::SporeBank => (*entry_id as usize) < world.spore_bank.len(),
            };
            known.then_some(()).ok_or("no such entry")
        }
    }
}

fn unhex<const N: usize>(s: &str) -> Option<[u8; N]> {
    if s.len() != 2 * N {
        return None;
    }
    let mut out = [0u8; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

fn unhex_vec(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || s.len() > 4096 {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok())
        .collect()
}

#[derive(Deserialize)]
struct Proposal {
    /// The canonical bytes of the wish, hex.
    wish: String,
    signature: String,
    /// The first spark, 72 bytes hex.
    spark: String,
}

/// `POST /v0/proposals`: a wish is created only with its first spark (spec §17).
async fn propose(
    State(api): State<Api>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(p): Json<Proposal>,
) -> Response {
    let ip = ip_key(addr.ip());
    {
        let mut g = GUARD.lock().expect("the lock is never poisoned");
        if g.banned(&ip) {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "E_BANNED", "message": "" })),
            )
                .into_response();
        }
        if !g.take(addr.ip()) {
            return refuse(Refusal::Limit);
        }
    }
    let (Some(bytes), Some(sig), Some(spark)) = (
        unhex_vec(&p.wish),
        unhex::<64>(&p.signature),
        unhex::<72>(&p.spark),
    ) else {
        return refuse(Refusal::Format);
    };
    let spark = Spark::from_bytes(&spark);
    let (w, ticket, previous) = {
        let intake = api.intake.lock().expect("the lock is never poisoned");
        let w = match intake.precheck_wish(&bytes, &sig) {
            Ok(w) => w,
            Err(r) => return refuse(r),
        };
        match intake.ticket() {
            Ok(t) => (w, t, intake.previous),
            Err(r) => return refuse(r),
        }
    };
    if spark.proposal_id != w.id() {
        return refuse(Refusal::ActionInvalid(
            "the first spark is for another wish",
        ));
    }
    {
        let live = api.live.read().expect("the lock is never poisoned");
        if let Err(why) = soft_check(&live.world, &w) {
            return refuse(Refusal::ActionInvalid(why));
        }
    }
    let Some(checked) = check_pow(ticket, previous, vec![spark]).await else {
        return overloaded();
    };
    let weight = match checked[0] {
        Pow::Valid(w) => w,
        Pow::Late => return refuse(Refusal::WindowClosed),
        Pow::Invalid => {
            ban(&ip, &spark.miner);
            return refuse(Refusal::PowInvalid);
        }
    };
    let mut intake = api.intake.lock().expect("the lock is never poisoned");
    if let Err(r) = intake.precheck_wish(&bytes, &sig) {
        return refuse(r);
    }
    if let Err(e) = intake.add_wish(&w, &bytes, &sig) {
        return internal(e);
    }
    match intake.append(&ticket, &[(spark, weight)]) {
        Ok(mut r) => match r.remove(0) {
            Ok(receipt) => {
                Json(json!({ "proposal_id": hex(&w.id()), "receipt": receipt_json(&receipt) }))
                    .into_response()
            }
            Err(r) => refuse(r),
        },
        Err(e) => internal(e),
    }
}

fn ban(ip: &[u8], miner: &[u8; 32]) {
    let mut g = GUARD.lock().expect("the lock is never poisoned");
    let until = Instant::now() + BAN;
    g.banned.insert(ip.to_vec(), until);
    g.banned.insert(miner.to_vec(), until);
}

/// `POST /v0/sparks`: a batch of up to 64 sparks, 72 bytes each, for the open window. The answer
/// has a receipt or an error for each spark, in order.
async fn sparks(
    State(api): State<Api>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    body: Bytes,
) -> Response {
    let ip = ip_key(addr.ip());
    let Some(batch) = parse_batch(&body) else {
        return refuse(Refusal::Format);
    };
    {
        let mut g = GUARD.lock().expect("the lock is never poisoned");
        if g.banned(&ip) || batch.iter().any(|s| g.banned(&s.miner)) {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "E_BANNED", "message": "" })),
            )
                .into_response();
        }
        if !g.take(addr.ip()) {
            return refuse(Refusal::Limit);
        }
    }
    let (ticket, previous, pre): (Ticket, Option<Ticket>, Vec<Result<(), Refusal>>) = {
        let intake = api.intake.lock().expect("the lock is never poisoned");
        let ticket = match intake.ticket() {
            Ok(t) => t,
            Err(r) => return refuse(r),
        };
        (
            ticket,
            intake.previous,
            batch.iter().map(|s| intake.precheck(s)).collect(),
        )
    };
    let to_check: Vec<Spark> = batch
        .iter()
        .zip(&pre)
        .filter(|(_, p)| p.is_ok())
        .map(|(s, _)| *s)
        .collect();
    let Some(checked) = check_pow(ticket, previous, to_check.clone()).await else {
        return overloaded();
    };
    let mut passed = Vec::new();
    let mut verdict: HashMap<Spark, Result<(), Refusal>> = HashMap::new();
    for (s, w) in to_check.iter().zip(&checked) {
        match w {
            Pow::Valid(w) => passed.push((*s, *w)),
            Pow::Late => {
                verdict.insert(*s, Err(Refusal::WindowClosed));
            }
            Pow::Invalid => {
                ban(&ip, &s.miner);
                verdict.insert(*s, Err(Refusal::PowInvalid));
            }
        }
    }
    let appended = {
        let mut intake = api.intake.lock().expect("the lock is never poisoned");
        match intake.append(&ticket, &passed) {
            Ok(r) => r,
            Err(e) => return internal(e),
        }
    };
    let mut receipts: HashMap<Spark, Value> = HashMap::new();
    for ((s, _), r) in passed.iter().zip(appended) {
        match r {
            Ok(receipt) => {
                receipts.insert(*s, json!({ "receipt": receipt_json(&receipt) }));
            }
            Err(r) => {
                verdict.insert(*s, Err(r));
            }
        }
    }
    let results: Vec<Value> = batch
        .iter()
        .zip(pre)
        .map(
            |(s, p)| match (p, verdict.get(s).copied(), receipts.get(s)) {
                (Err(r), _, _) | (Ok(()), Some(Err(r)), _) => json!({ "error": r.code() }),
                (Ok(()), _, Some(v)) => v.clone(),
                _ => json!({ "error": "E_INTERNAL" }),
            },
        )
        .collect();
    Json(json!({ "epoch": ticket.epoch, "results": results })).into_response()
}

#[derive(Deserialize)]
struct ProposalQuery {
    status: Option<String>,
    limit: Option<u32>,
}

async fn proposals(State(api): State<Api>, Query(q): Query<ProposalQuery>) -> Response {
    let intake = api.intake.lock().expect("the lock is never poisoned");
    match intake.wishes(q.status.as_deref(), q.limit.unwrap_or(100).min(1000)) {
        Ok(rows) => Json(json!({ "proposals": rows })).into_response(),
        Err(e) => internal(e),
    }
}

/// The price of a miracle for the next selection and its floor (spec §19), in work units, as
/// strings (they may exceed 2^53).
async fn ledger(State(api): State<Api>) -> Response {
    let intake = api.intake.lock().expect("the lock is never poisoned");
    match intake.price() {
        Ok(p) => Json(json!({
            "price": p.to_string(),
            "price_min": intake.price_min.to_string(),
            "price_mult": { "weather": 100, "migrate": 120, "revive": 200 },
            "per_epoch": crate::intake::PER_EPOCH,
        }))
        .into_response(),
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
struct RangeQuery {
    from: Option<u64>,
    to: Option<u64>,
}

/// The miracles given to the world, by epoch, with their outcomes: what the time machine and
/// any watcher need to replay the world.
async fn miracles(State(api): State<Api>, Query(q): Query<RangeQuery>) -> Response {
    let intake = api.intake.lock().expect("the lock is never poisoned");
    let from = q.from.unwrap_or(0);
    match intake.miracles(from, q.to.unwrap_or(u64::MAX / 2)) {
        Ok(rows) => Json(json!({ "miracles": rows })).into_response(),
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
struct EpochQuery {
    epoch: Option<u64>,
}

/// The latest signed tree head of an epoch's spark log (the open window's by default).
async fn sth(State(api): State<Api>, Query(q): Query<EpochQuery>) -> Response {
    let intake = api.intake.lock().expect("the lock is never poisoned");
    match intake.head(q.epoch.unwrap_or(intake.epoch)) {
        Ok(Some(s)) => Json(sth_json(&s)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "E_NOT_FOUND", "message": "no tree head" })),
        )
            .into_response(),
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
struct InclusionQuery {
    index: u64,
    size: u64,
}

async fn inclusion(
    State(api): State<Api>,
    Path(epoch): Path<u64>,
    Query(q): Query<InclusionQuery>,
) -> Response {
    let intake = api.intake.lock().expect("the lock is never poisoned");
    proof_reply(intake.inclusion(epoch, q.index, q.size))
}

#[derive(Deserialize)]
struct ConsistencyQuery {
    first: u64,
    second: u64,
}

async fn consistency(
    State(api): State<Api>,
    Path(epoch): Path<u64>,
    Query(q): Query<ConsistencyQuery>,
) -> Response {
    let intake = api.intake.lock().expect("the lock is never poisoned");
    proof_reply(intake.consistency(epoch, q.first, q.second))
}

fn proof_reply(p: Result<Option<Vec<Hash>>, String>) -> Response {
    match p {
        Ok(Some(path)) => {
            Json(json!({ "path": path.iter().map(|h| hex(h)).collect::<Vec<_>>() })).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "E_NOT_FOUND", "message": "no such tree" })),
        )
            .into_response(),
        Err(e) => internal(e),
    }
}
