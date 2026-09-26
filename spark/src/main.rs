//! The spark client (stage C): a naturalist's key, wishes and the sparks that carry them.
//!
//!   protogaea-spark key                                   make a key (or show it)
//!   protogaea-spark wish weather X Y rain|drought         make a wish with its first spark
//!   protogaea-spark wish migrate CLADE FROM_X FROM_Y TO_X TO_Y
//!   protogaea-spark wish revive museum|spores ENTRY X Y [i:j ...]
//!   protogaea-spark mine PROPOSAL_ID [--threads N] [--minutes M]
//!
//! Options: --server URL (default http://127.0.0.1:8081), --key FILE (default protogaea-spark.key).
//! The server's password, if it has one, comes from PROTOGAEA_USER and PROTOGAEA_PASSWORD.
//!
//! Every receipt is checked against the operator's key: the spark must be in the signed tree.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use protogaea_pow::{meets, spark_input, weight, SPARK};
use protogaea_protocol::spark::{Spark, MAX_BATCH};
use protogaea_protocol::sth::{Receipt, Sth};
use protogaea_protocol::wish::{self, Action, Source, Weather, Wish, MAX_LIFETIME};
use protogaea_protocol::{hex, Hash};
use serde_json::{json, Value};

type Result<T> = std::result::Result<T, String>;

struct Client {
    server: String,
    auth: Option<String>,
}

impl Client {
    fn get(&self, path: &str) -> Result<Value> {
        let mut r = ureq::get(&format!("{}{path}", self.server));
        if let Some(a) = &self.auth {
            r = r.header("Authorization", a);
        }
        let mut res = r.call().map_err(|e| format!("GET {path}: {e}"))?;
        res.body_mut().read_json().map_err(|e| e.to_string())
    }

    fn post(&self, path: &str, content_type: &str, body: &[u8]) -> Result<Value> {
        let url = format!("{}{path}", self.server);
        let mut r = ureq::post(&url).header("Content-Type", content_type);
        if let Some(a) = &self.auth {
            r = r.header("Authorization", a);
        }
        let mut res = r
            .config()
            .http_status_as_error(false)
            .build()
            .send(body)
            .map_err(|e| format!("POST {path}: {e}"))?;
        let status = res.status();
        let v: Value = res.body_mut().read_json().map_err(|e| e.to_string())?;
        if status.is_success() {
            Ok(v)
        } else {
            Err(format!(
                "{}: {} {}",
                status,
                v["error"].as_str().unwrap_or("?"),
                v["message"].as_str().unwrap_or("")
            ))
        }
    }
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = u32::from(c[0]) << 16
            | u32::from(*c.get(1).unwrap_or(&0)) << 8
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn unhex<const N: usize>(s: &str) -> Result<[u8; N]> {
    if s.len() != 2 * N {
        return Err(format!("expected {N} bytes of hex"));
    }
    let mut out = [0u8; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// The open window as the miner needs it.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Window {
    epoch: u64,
    challenge: Hash,
    target: u64,
    world_id: [u8; 16],
}

fn window(c: &Client) -> Result<(Window, Value)> {
    let w = c.get("/v0/window")?;
    if w["open"] != true {
        return Err("the window is closed; try again in a few seconds".into());
    }
    let target: u64 = w["target"]
        .as_str()
        .ok_or("no target")?
        .parse()
        .map_err(|_| "a bad target")?;
    Ok((
        Window {
            epoch: w["epoch"].as_u64().ok_or("no epoch")?,
            challenge: unhex(w["challenge"].as_str().ok_or("no challenge")?)?,
            target,
            world_id: unhex(w["world_id"].as_str().ok_or("no world")?)?,
        },
        w,
    ))
}

/// Checks a receipt from the server against the operator's key.
fn check_receipt(v: &Value, spark: &Spark, operator: &[u8; 32]) -> Result<Receipt> {
    let s = &v["sth"];
    let sth = Sth {
        epoch: s["epoch"].as_u64().ok_or("no epoch")?,
        tree_size: s["tree_size"].as_u64().ok_or("no size")?,
        root: unhex(s["root"].as_str().ok_or("no root")?)?,
        timestamp_ms: s["timestamp_ms"].as_u64().ok_or("no time")?,
        signature: unhex(s["signature"].as_str().ok_or("no signature")?)?,
    };
    let path = v["path"]
        .as_array()
        .ok_or("no path")?
        .iter()
        .map(|h| unhex::<32>(h.as_str().unwrap_or("")))
        .collect::<Result<Vec<_>>>()?;
    let r = Receipt {
        spark: *spark,
        leaf_index: v["leaf_index"].as_u64().ok_or("no index")?,
        sth,
        path,
    };
    if r.verify(operator) {
        Ok(r)
    } else {
        Err("a receipt that does not verify against the operator's key".into())
    }
}

fn hasher() -> impl FnMut(&[u8]) -> [u8; 32] {
    #[cfg(feature = "c")]
    let mut h = protogaea_pow::c::Hasher::new(SPARK);
    #[cfg(not(feature = "c"))]
    let mut h = protogaea_pow::Hasher::new(SPARK);
    move |input| h.hash(input)
}

/// Mines sparks for a wish on `threads` threads until `stop`; found sparks come out of the channel
/// with the window they are for. The window is re-read by the caller and shared here.
fn start_miners(
    threads: usize,
    proposal_id: Hash,
    miner: [u8; 32],
    window: Arc<RwLock<Window>>,
    stop: Arc<AtomicBool>,
    hashes: Arc<AtomicU64>,
) -> mpsc::Receiver<(Window, Spark)> {
    let (tx, rx) = mpsc::channel();
    let mut start = [0u8; 8];
    getrandom::fill(&mut start).expect("randomness");
    let base = u64::from_le_bytes(start);
    for t in 0..threads {
        let (tx, window, stop, hashes) = (tx.clone(), window.clone(), stop.clone(), hashes.clone());
        std::thread::spawn(move || {
            let mut hash = hasher();
            let mut nonce = base.wrapping_add((t as u64) << 40);
            while !stop.load(Ordering::Relaxed) {
                let w = *window.read().expect("the lock is never poisoned");
                let input = spark_input(
                    &w.world_id,
                    w.epoch,
                    &w.challenge,
                    &proposal_id,
                    &miner,
                    nonce,
                );
                if meets(&hash(&input), w.target) {
                    let _ = tx.send((
                        w,
                        Spark {
                            proposal_id,
                            miner,
                            nonce,
                        },
                    ));
                }
                hashes.fetch_add(1, Ordering::Relaxed);
                nonce = nonce.wrapping_add(1);
            }
        });
    }
    rx
}

fn key(path: &str) -> Result<[u8; 32]> {
    match std::fs::read(path) {
        Ok(b) => b
            .try_into()
            .map_err(|_| format!("{path} is not a 32-byte key")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut k = [0u8; 32];
            getrandom::fill(&mut k).map_err(|e| e.to_string())?;
            std::fs::write(path, k).map_err(|e| format!("cannot write {path}: {e}"))?;
            eprintln!("made a new key in {path}; keep it, it is your naturalist identity");
            Ok(k)
        }
        Err(e) => Err(format!("cannot read {path}: {e}")),
    }
}

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn run() -> Result<()> {
    let all: Vec<String> = std::env::args().skip(1).collect();
    // Positional arguments, without the `--option value` pairs.
    let mut args: Vec<String> = Vec::new();
    let mut i = 0;
    while i < all.len() {
        if all[i].starts_with("--") {
            i += 2;
        } else {
            args.push(all[i].clone());
            i += 1;
        }
    }
    let client = Client {
        server: arg(&all, "--server")
            .unwrap_or("http://127.0.0.1:8081")
            .trim_end_matches('/')
            .to_string(),
        auth: std::env::var("PROTOGAEA_PASSWORD").ok().map(|p| {
            let u = std::env::var("PROTOGAEA_USER").unwrap_or_else(|_| "protogaea".into());
            format!("Basic {}", base64(format!("{u}:{p}").as_bytes()))
        }),
    };
    let secret = key(arg(&all, "--key").unwrap_or("protogaea-spark.key"))?;
    let me = wish::public_key(&secret);
    let threads: usize = arg(&all, "--threads")
        .and_then(|t| t.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()) / 2)
        .max(1);
    match args.first().map(String::as_str) {
        Some("key") => {
            println!("naturalist key: {}", hex(&me));
            Ok(())
        }
        Some("wish") => {
            let n = |i: usize, what: &str| -> Result<u32> {
                args.get(i)
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| format!("{what}: a number"))
            };
            let action = match args.get(1).map(String::as_str) {
                Some("weather") => Action::Weather {
                    x: n(2, "X")? as u8,
                    y: n(3, "Y")? as u8,
                    kind: match args.get(4).map(String::as_str) {
                        Some("rain") => Weather::Rain,
                        Some("drought") => Weather::Drought,
                        _ => return Err("rain or drought".into()),
                    },
                },
                Some("migrate") => Action::Migrate {
                    clade_id: n(2, "CLADE")?,
                    from: (n(3, "FROM_X")? as u8, n(4, "FROM_Y")? as u8),
                    to: (n(5, "TO_X")? as u8, n(6, "TO_Y")? as u8),
                },
                Some("revive") => Action::Revive {
                    source: match args.get(2).map(String::as_str) {
                        Some("museum") => Source::Museum,
                        Some("spores") => Source::SporeBank,
                        _ => return Err("museum or spores".into()),
                    },
                    entry_id: n(3, "ENTRY")?,
                    at: (n(4, "X")? as u8, n(5, "Y")? as u8),
                    // Edit steps as i:j, each moving one point from trait i to trait j.
                    steps: args[6.min(args.len())..]
                        .iter()
                        .map(|s| {
                            let (i, j) = s.split_once(':').ok_or("a step is i:j")?;
                            Ok((i.parse().map_err(|_| "i")?, j.parse().map_err(|_| "j")?))
                        })
                        .collect::<Result<Vec<(u8, u8)>>>()?,
                },
                _ => return Err("wish weather X Y rain|drought | wish migrate CLADE FROM_X FROM_Y TO_X TO_Y | wish revive museum|spores ENTRY X Y [i:j ...]".into()),
            };
            let operator: [u8; 32] = unhex(client.get("/v0/operator")?["operator"].as_str().ok_or("no operator")?)?;
            let (w, raw) = window(&client)?;
            let wish = Wish {
                world_id: w.world_id,
                ruleset_id: unhex(raw["ruleset_id"].as_str().ok_or("no ruleset")?)?,
                action,
                author: me,
                created_epoch: w.epoch,
                expires_epoch: w.epoch + MAX_LIFETIME,
                hypothesis: None,
                name: None,
            };
            let id = wish.id();
            let signature = wish::sign(&secret, &id);
            println!("wish {} for epoch {}, mining its first spark on {threads} threads…", hex(&id), w.epoch);
            let (window, stop, hashes) = (Arc::new(RwLock::new(w)), Arc::new(AtomicBool::new(false)), Arc::new(AtomicU64::new(0)));
            let rx = start_miners(threads, id, me, window, stop.clone(), hashes);
            let (_, spark) = rx.recv().map_err(|e| e.to_string())?;
            stop.store(true, Ordering::Relaxed);
            let body = json!({
                "wish": hex(&wish.to_bytes()),
                "signature": hex(&signature),
                "spark": hex(&spark.to_bytes()),
            });
            let res = client.post("/v0/proposals", "application/json", body.to_string().as_bytes())?;
            check_receipt(&res["receipt"], &spark, &operator)?;
            println!("accepted: proposal {} (receipt checked)", res["proposal_id"].as_str().unwrap_or("?"));
            Ok(())
        }
        Some("mine") => {
            let id: Hash = unhex(args.get(1).ok_or("the proposal id")?)?;
            let minutes: f64 = arg(&all, "--minutes").and_then(|m| m.parse().ok()).unwrap_or(1.0);
            let operator: [u8; 32] = unhex(client.get("/v0/operator")?["operator"].as_str().ok_or("no operator")?)?;
            let (w, _) = window(&client)?;
            let shared = Arc::new(RwLock::new(w));
            let (stop, hashes) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicU64::new(0)));
            let rx = start_miners(threads, id, me, shared.clone(), stop.clone(), hashes.clone());
            let (started, until) = (Instant::now(), Instant::now() + Duration::from_secs_f64(minutes * 60.0));
            let (mut accepted, mut work, mut last_check) = (0u64, 0u128, Instant::now());
            println!("mining for {} on {threads} threads, {minutes} min…", hex(&id));
            while Instant::now() < until {
                let mut batch: Vec<(Window, Spark)> = Vec::new();
                let flush_at = Instant::now() + Duration::from_secs(3);
                while batch.len() < MAX_BATCH && Instant::now() < flush_at {
                    if let Ok(s) = rx.recv_timeout(Duration::from_millis(200)) {
                        batch.push(s);
                    }
                }
                if last_check.elapsed() > Duration::from_secs(5) {
                    last_check = Instant::now();
                    if let Ok((w, _)) = window(&client) {
                        *shared.write().expect("the lock is never poisoned") = w;
                    }
                }
                let current = *shared.read().expect("the lock is never poisoned");
                batch.retain(|(w, _)| *w == current);
                if batch.is_empty() {
                    continue;
                }
                let body: Vec<u8> = batch.iter().flat_map(|(_, s)| s.to_bytes()).collect();
                match client.post("/v0/sparks", "application/octet-stream", &body) {
                    Ok(res) => {
                        for ((w, s), r) in batch.iter().zip(res["results"].as_array().into_iter().flatten()) {
                            if r.get("receipt").is_some() {
                                check_receipt(&r["receipt"], s, &operator)?;
                                accepted += 1;
                                work += u128::from(weight(w.target));
                            } else {
                                eprintln!("refused: {}", r["error"]);
                            }
                        }
                    }
                    Err(e) => eprintln!("{e}"),
                }
                let secs = started.elapsed().as_secs_f64();
                println!(
                    "epoch {}: {accepted} sparks accepted, {work} work units, {:.0} H/s",
                    current.epoch,
                    hashes.load(Ordering::Relaxed) as f64 / secs
                );
            }
            stop.store(true, Ordering::Relaxed);
            Ok(())
        }
        _ => Err("usage: protogaea-spark key | wish weather X Y rain|drought | wish migrate CLADE FROM_X FROM_Y TO_X TO_Y | wish revive museum|spores ENTRY X Y [i:j ...] | mine PROPOSAL_ID [--threads N] [--minutes M] [--server URL] [--key FILE]".into()),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
