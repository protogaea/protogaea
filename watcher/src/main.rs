//! An independent watcher of a Protogaea world (spec §21). It trusts nothing the server says that
//! it can check itself:
//!
//! - **the spark log:** every tree head it sees is signed by the operator, and each one extends
//!   the one before (a consistency proof it verifies); a log that shrinks or is rewritten is an
//!   alarm, because a receipt given earlier could then be dropped;
//! - **the headers:** each signed epoch header checks against the operator key and its own hash,
//!   chains to the previous one, and commits to a final tree head consistent with every head the
//!   watcher saw during the window;
//! - **the world:** it replays the world from genesis with the miracles the server logged, and the
//!   `state_root` of every epoch must equal the one in the log (the signed header where there is
//!   one).
//!
//! Every alarm is printed and appended to `--report` (JSON lines). With `--once` it checks what is
//! there and exits: status 0 if all holds, 1 on any alarm.
//!
//!   protogaea-watcher [--server URL] [--report FILE] [--once]

use std::collections::BTreeMap;
use std::io::Write as _;
use std::time::{Duration, Instant};

use protogaea_core::run::{hex, Run};
use protogaea_core::{Miracle, Ruleset};
use protogaea_protocol::header::Header;
use protogaea_protocol::log::verify_consistency;
use protogaea_protocol::sth::Sth;
use protogaea_protocol::Hash;
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
        let mut res = r
            .config()
            .http_status_as_error(false)
            .build()
            .call()
            .map_err(|e| format!("GET {path}: {e}"))?;
        let status = res.status();
        let v: Value = res
            .body_mut()
            .read_json()
            .map_err(|e| format!("GET {path}: {e}"))?;
        if status.is_success() {
            Ok(v)
        } else if status == 404 {
            Ok(Value::Null)
        } else {
            Err(format!("GET {path}: {status}"))
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
            out.push(if i <= c.len() {
                T[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

fn unhex<const N: usize>(v: &Value) -> Result<[u8; N]> {
    let s = v.as_str().ok_or("expected hex")?;
    if s.len() != 2 * N {
        return Err(format!("expected {N} bytes of hex"));
    }
    let mut out = [0u8; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

fn sth_of(v: &Value) -> Result<Sth> {
    Ok(Sth {
        epoch: v["epoch"].as_u64().ok_or("no epoch")?,
        tree_size: v["tree_size"].as_u64().ok_or("no size")?,
        root: unhex(&v["root"])?,
        timestamp_ms: v["timestamp_ms"].as_u64().ok_or("no time")?,
        signature: unhex(&v["signature"])?,
    })
}

fn header_of(v: &Value) -> Result<(Header, Hash, [u8; 64])> {
    Ok((
        Header {
            epoch: v["epoch"].as_u64().ok_or("no epoch")?,
            prev_header_hash: unhex(&v["prev_header_hash"])?,
            ruleset_id: unhex(&v["ruleset_id"])?,
            state_root: unhex(&v["state_root"])?,
            ledger_root: unhex(&v["ledger_root"])?,
            sth_size: v["sth_size"].as_u64().ok_or("no size")?,
            sth_root: unhex(&v["sth_root"])?,
            beacon: unhex(&v["beacon"])?,
            miracles_root: unhex(&v["miracles_root"])?,
            timestamp_ms: v["timestamp_ms"].as_u64().ok_or("no time")?,
        },
        unhex(&v["hash"])?,
        unhex(&v["signature"])?,
    ))
}

struct Watcher {
    client: Client,
    operator: [u8; 32],
    report: Option<std::fs::File>,
    alarms: u32,
    /// The largest signed tree head seen for each epoch still being watched.
    heads: BTreeMap<u64, Sth>,
    /// The last signed header checked, and its hash.
    last_header: Option<(u64, Hash)>,
    /// The replayed world and the next epoch to compare.
    run: Run,
    /// Once the replay has diverged from the log, later epochs are not compared again.
    diverged: bool,
    /// Alarms already raised, so that one found at every poll is reported once.
    raised: std::collections::HashSet<String>,
}

impl Watcher {
    fn alarm(&mut self, what: &str, detail: Value) {
        if !self.raised.insert(format!("{what} {detail}")) {
            return;
        }
        self.alarms += 1;
        eprintln!("ALARM {what}: {detail}");
        if let Some(f) = &mut self.report {
            let _ = writeln!(
                f,
                "{}",
                json!({ "alarm": what, "detail": detail, "at_ms": now_ms() })
            );
        }
    }

    /// A tree head of the open window: signed, and consistent with the last one seen.
    fn check_head(&mut self) -> Result<()> {
        let v = self.client.get("/v0/sth")?;
        if v.is_null() {
            return Ok(());
        }
        let sth = sth_of(&v)?;
        if !sth.verify(&self.operator) {
            self.alarm("a tree head not signed by the operator", v);
            return Ok(());
        }
        self.extends(sth, "tree head")
    }

    /// Checks that `sth` extends the largest head seen for its epoch, and keeps the larger one.
    fn extends(&mut self, sth: Sth, what: &str) -> Result<()> {
        if let Some(seen) = self.heads.get(&sth.epoch).copied() {
            if sth.tree_size < seen.tree_size {
                self.alarm(
                    "the spark log shrank",
                    json!({ "what": what, "epoch": sth.epoch, "seen": seen.tree_size, "now": sth.tree_size }),
                );
                return Ok(());
            }
            if sth.tree_size == seen.tree_size && sth.root != seen.root {
                self.alarm(
                    "the spark log was rewritten",
                    json!({ "what": what, "epoch": sth.epoch, "size": sth.tree_size }),
                );
                return Ok(());
            }
            if sth.tree_size > seen.tree_size && seen.tree_size > 0 {
                let p = self.client.get(&format!(
                    "/v0/log/{}/consistency?first={}&second={}",
                    sth.epoch, seen.tree_size, sth.tree_size
                ))?;
                let path = p["path"]
                    .as_array()
                    .map(|a| a.iter().map(unhex::<32>).collect::<Result<Vec<_>>>())
                    .transpose()?
                    .unwrap_or_default();
                if !verify_consistency(seen.tree_size, sth.tree_size, &seen.root, &sth.root, &path)
                {
                    self.alarm(
                        "a tree head does not extend the one before",
                        json!({ "what": what, "epoch": sth.epoch, "from": seen.tree_size, "to": sth.tree_size }),
                    );
                    return Ok(());
                }
            }
        }
        if self
            .heads
            .get(&sth.epoch)
            .is_none_or(|s| sth.tree_size >= s.tree_size)
        {
            self.heads.insert(sth.epoch, sth);
        }
        Ok(())
    }

    /// The signed headers after the last one checked: signature, hash, chain, and a final tree
    /// head consistent with the heads seen in the window.
    fn check_headers(&mut self) -> Result<u32> {
        let from = self.last_header.map_or(0, |(e, _)| e + 1);
        let list = self
            .client
            .get(&format!("/v0/headers?from={from}&to={}", from + 499))?;
        let mut n = 0;
        for v in list["headers"].as_array().cloned().unwrap_or_default() {
            let (h, hash, sig) = header_of(&v)?;
            n += 1;
            if h.hash() != hash {
                self.alarm(
                    "a header whose hash does not match its fields",
                    json!({ "epoch": h.epoch }),
                );
            }
            if !h.verify(&self.operator, &sig) {
                self.alarm(
                    "a header not signed by the operator",
                    json!({ "epoch": h.epoch }),
                );
            }
            if let Some((e, prev)) = self.last_header {
                if h.prev_header_hash != prev {
                    self.alarm(
                        "the header chain is broken",
                        json!({ "epoch": h.epoch, "previous": e }),
                    );
                }
            }
            // The final tree head the header commits to must extend what was seen in the window.
            if h.sth_size > 0 {
                let signed = Sth {
                    epoch: h.epoch,
                    tree_size: h.sth_size,
                    root: h.sth_root,
                    timestamp_ms: 0,
                    signature: [0; 64],
                };
                self.extends(signed, "the header's final tree head")?;
            } else if self.heads.get(&h.epoch).is_some_and(|s| s.tree_size > 0) {
                self.alarm(
                    "a header drops the epoch's sparks",
                    json!({ "epoch": h.epoch }),
                );
            }
            self.heads.retain(|&e, _| e > h.epoch);
            self.last_header = Some((h.epoch, hash));
        }
        Ok(n)
    }

    /// Replays the world up to the latest epoch, with the logged miracles, comparing every root
    /// with the log (the signed header where there is one). Returns the epochs replayed.
    fn replay(&mut self) -> Result<u64> {
        if self.diverged {
            return Ok(0);
        }
        let latest = self.client.get("/v0/world")?["header"]["epoch"]
            .as_u64()
            .ok_or("no epoch")?;
        let mut done = 0;
        while self.run.world.epoch < latest {
            let from = self.run.world.epoch + 1;
            let to = latest.min(from + 99);
            let logged = self
                .client
                .get(&format!("/v0/epochs?from={from}&to={to}&limit=100"))?;
            let signed = self
                .client
                .get(&format!("/v0/headers?from={from}&to={to}"))?;
            let miracles = self
                .client
                .get(&format!("/v0/miracles?from={from}&to={to}"))?;
            let root_in = |list: &Value, key: &str, e: u64| -> Option<String> {
                list[key]
                    .as_array()?
                    .iter()
                    .find(|h| h["epoch"].as_u64() == Some(e))
                    .and_then(|h| h["state_root"].as_str().map(str::to_string))
            };
            for e in from..=to {
                let given: Vec<Miracle> = miracles["miracles"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|m| m["epoch"].as_u64() == Some(e))
                    .map(|m| {
                        serde_json::from_value(m["miracle"].clone()).map_err(|x| x.to_string())
                    })
                    .collect::<Result<_>>()?;
                self.run.step_with(&given);
                done += 1;
                let mine = hex(&self.run.state_root());
                let theirs =
                    root_in(&signed, "headers", e).or_else(|| root_in(&logged, "epochs", e));
                match theirs {
                    Some(r) if r == mine => {}
                    Some(r) => {
                        self.alarm(
                            "the world does not replay to the logged root",
                            json!({ "epoch": e, "replayed": mine, "logged": r }),
                        );
                        self.diverged = true;
                        return Ok(done);
                    }
                    None => {}
                }
            }
        }
        Ok(done)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn run() -> Result<u32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let client = Client {
        server: arg(&args, "--server")
            .unwrap_or("http://127.0.0.1:8081")
            .trim_end_matches('/')
            .to_string(),
        auth: std::env::var("PROTOGAEA_PASSWORD").ok().map(|p| {
            let u = std::env::var("PROTOGAEA_USER").unwrap_or_else(|_| "protogaea".into());
            format!("Basic {}", base64(format!("{u}:{p}").as_bytes()))
        }),
    };
    let once = args.iter().any(|a| a == "--once");
    let report = arg(&args, "--report")
        .map(|p| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .map_err(|e| format!("{p}: {e}"))
        })
        .transpose()?;
    let operator: [u8; 32] = unhex(&client.get("/v0/operator")?["operator"])?;
    let world = client.get("/v0/world")?;
    let rules: Ruleset = serde_json::from_value(client.get("/v0/ruleset")?)
        .map_err(|e| format!("the ruleset: {e}"))?;
    let seed = world["seed"].as_u64().ok_or("no seed")?;
    let mut run = Run::new(seed, rules.clone());
    if hex(&run.world.world_id) != world["world_id"].as_str().unwrap_or("") {
        return Err("the world id does not follow from the seed: another world or rules".into());
    }
    // Genesis must give the logged root of epoch 0. A world born before its rules gained a field
    // has another `ruleset_id` (the hash of the rules as they were serialized then): genesis with
    // the id the world declares must then match, and everything after it is checked as usual.
    let genesis_root = client.get("/v0/epochs?from=0&to=0&limit=1")?["epochs"][0]["state_root"]
        .as_str()
        .map(str::to_string);
    if let Some(logged) = genesis_root {
        if hex(&run.state_root()) != logged {
            let declared: [u8; 32] = unhex(&world["ruleset_id"])?;
            let mut w0 = run.world.clone();
            w0.ruleset_id = declared;
            let again = Run::resume(seed, rules, w0);
            if hex(&again.state_root()) == logged {
                eprintln!(
                    "note: the world's ruleset_id {} is not the hash of its rules today ({}); the world predates a change to the rules' format",
                    hex(&declared),
                    hex(&run.world.ruleset_id)
                );
                run = again;
            } else {
                return Err(
                    "genesis from the seed and the rules does not give the logged root of epoch 0"
                        .into(),
                );
            }
        }
    }
    println!(
        "watching world {} (seed {seed}), operator {}",
        hex(&run.world.world_id),
        hex(&operator)
    );
    let mut w = Watcher {
        client,
        operator,
        report,
        alarms: 0,
        heads: BTreeMap::new(),
        last_header: None,
        run,
        diverged: false,
        raised: Default::default(),
    };
    let mut last_replay = Instant::now() - Duration::from_secs(3600);
    loop {
        w.check_head()?;
        if once || last_replay.elapsed() > Duration::from_secs(20) {
            last_replay = Instant::now();
            let headers = w.check_headers()?;
            let started = Instant::now();
            let replayed = w.replay()?;
            if headers > 0 || replayed > 0 {
                println!(
                    "epoch {}: {headers} signed headers checked, {replayed} epochs replayed in {:.1} s, {} alarms",
                    w.run.world.epoch,
                    started.elapsed().as_secs_f64(),
                    w.alarms
                );
            }
        }
        if once {
            return Ok(w.alarms);
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn main() {
    match run() {
        Ok(0) => println!("all holds"),
        Ok(n) => {
            eprintln!("{n} alarms");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    }
}
