//! A load test for spark intake (spec §29): honest miners and a flood of forged sparks at once,
//! each client from its own loopback address, so the server's limits per address and subnet see
//! them apart. Run it next to a test server (Linux binds any 127.x.y.z):
//!
//!   cargo run --release -p protogaea-spark --features c --example load -- \
//!     --server 127.0.0.1:8095 --proposal ID [--honest N] [--honest-rate SPARKS_PER_S]
//!     [--batch K] [--flood-addrs A] [--flood-rate SPARKS_PER_S] [--flood-per-subnet P]
//!     [--seconds S] [--pid SERVER_PID]
//!
//! Honest clients mine real sparks for the wish and send them in batches; the flood sends random
//! nonces for it (almost all forged at the server's target) from `A` addresses. With `--pid` the
//! server's CPU use is read from /proc. The report: sparks accepted per second, the honest
//! latency, the answers by code, and the server's cores in use.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use protogaea_pow::{meets, spark_input, SPARK};
use protogaea_protocol::spark::Spark;
use protogaea_protocol::Hash;
use serde_json::Value;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

type Result<T> = std::result::Result<T, String>;

/// A keep-alive HTTP/1.1 connection from one source address.
struct Conn {
    src: Ipv4Addr,
    server: SocketAddr,
    stream: Option<BufReader<TcpStream>>,
}

impl Conn {
    fn new(src: Ipv4Addr, server: SocketAddr) -> Conn {
        Conn {
            src,
            server,
            stream: None,
        }
    }

    fn connect(&self) -> Result<BufReader<TcpStream>> {
        let s = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
            .map_err(|e| e.to_string())?;
        s.bind(&SockAddr::from(SocketAddr::new(IpAddr::V4(self.src), 0)))
            .map_err(|e| format!("bind {}: {e}", self.src))?;
        s.connect_timeout(&SockAddr::from(self.server), Duration::from_secs(5))
            .map_err(|e| format!("connect: {e}"))?;
        let t: TcpStream = s.into();
        t.set_nodelay(true).map_err(|e| e.to_string())?;
        t.set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| e.to_string())?;
        Ok(BufReader::new(t))
    }

    fn request(&mut self, method: &str, path: &str, body: &[u8]) -> Result<(u16, Vec<u8>)> {
        let mut last = String::new();
        for _ in 0..2 {
            if self.stream.is_none() {
                self.stream = Some(self.connect()?);
            }
            match self.exchange(method, path, body) {
                Ok(r) => return Ok(r),
                Err(e) => {
                    self.stream = None;
                    last = e;
                }
            }
        }
        Err(last)
    }

    fn exchange(&mut self, method: &str, path: &str, body: &[u8]) -> Result<(u16, Vec<u8>)> {
        let r = self.stream.as_mut().expect("connected");
        let head = format!(
            "{method} {path} HTTP/1.1\r\nHost: load\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
            body.len()
        );
        let w = r.get_mut();
        w.write_all(head.as_bytes()).map_err(|e| e.to_string())?;
        w.write_all(body).map_err(|e| e.to_string())?;
        let mut line = String::new();
        r.read_line(&mut line).map_err(|e| e.to_string())?;
        let status: u16 = line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| format!("a bad status line {line:?}"))?;
        let mut length = 0usize;
        let mut close = false;
        loop {
            line.clear();
            r.read_line(&mut line).map_err(|e| e.to_string())?;
            let l = line.trim_end();
            if l.is_empty() {
                break;
            }
            let lower = l.to_ascii_lowercase();
            if let Some(v) = lower.strip_prefix("content-length:") {
                length = v.trim().parse().map_err(|_| "a bad length")?;
            }
            if lower.starts_with("connection:") && lower.contains("close") {
                close = true;
            }
        }
        let mut out = vec![0u8; length];
        r.read_exact(&mut out).map_err(|e| e.to_string())?;
        if close {
            self.stream = None;
        }
        Ok((status, out))
    }
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

#[derive(Clone, Copy, PartialEq, Eq)]
struct Window {
    epoch: u64,
    challenge: Hash,
    target: u64,
    world_id: [u8; 16],
    open: bool,
}

fn read_window(c: &mut Conn) -> Result<Window> {
    let (status, body) = c.request("GET", "/v0/window", &[])?;
    if status != 200 {
        return Err(format!("window: {status}"));
    }
    let w: Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    Ok(Window {
        epoch: w["epoch"].as_u64().ok_or("no epoch")?,
        challenge: unhex(w["challenge"].as_str().ok_or("no challenge")?)?,
        target: w["target"]
            .as_str()
            .ok_or("no target")?
            .parse()
            .map_err(|_| "a bad target")?,
        world_id: unhex(w["world_id"].as_str().ok_or("no world")?)?,
        open: w["open"] == true,
    })
}

fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).expect("randomness");
    b
}

/// What the clients saw, shared.
#[derive(Default)]
struct Tally {
    /// Per-spark answers of the honest clients: "receipt" or the error code.
    honest: BTreeMap<String, u64>,
    /// HTTP answers to the flood, by status.
    flood: BTreeMap<u16, u64>,
    /// Sparks the flood sent.
    flood_sparks: u64,
    /// Honest request latencies, in ms.
    latency: Vec<f64>,
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn num<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> Result<T> {
    arg(args, name).map_or(Ok(default), |v| {
        v.parse().map_err(|_| format!("{name}: a bad number"))
    })
}

/// User and system CPU seconds of a process, from /proc (Linux).
fn cpu_seconds(pid: u32) -> Option<f64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = &stat[stat.rfind(')')? + 2..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    let ticks: f64 = f.get(11)?.parse::<f64>().ok()? + f.get(12)?.parse::<f64>().ok()?;
    Some(ticks / 100.0)
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let server: SocketAddr = arg(&args, "--server")
        .unwrap_or_else(|| "127.0.0.1:8095".into())
        .parse()
        .map_err(|_| "--server: HOST:PORT")?;
    let proposal: Hash = unhex(&arg(&args, "--proposal").ok_or("--proposal ID is needed")?)?;
    let honest: usize = num(&args, "--honest", 4)?;
    let honest_rate: f64 = num(&args, "--honest-rate", 70.0)?;
    let batch: usize = num(&args, "--batch", 16)?;
    let flood_addrs: usize = num(&args, "--flood-addrs", 0)?;
    let flood_rate: f64 = num(&args, "--flood-rate", 10_000.0)?;
    let per_subnet: usize = num(&args, "--flood-per-subnet", 1)?;
    let seconds: f64 = num(&args, "--seconds", 60.0)?;
    let pid: Option<u32> = arg(&args, "--pid").and_then(|p| p.parse().ok());

    let mut probe = Conn::new(Ipv4Addr::new(127, 0, 0, 1), server);
    let window = Arc::new(RwLock::new(read_window(&mut probe)?));
    let stop = Arc::new(AtomicBool::new(false));
    let tally = Arc::new(Mutex::new(Tally::default()));
    let mut threads = Vec::new();

    // Honest miners: each mines its share of the rate and sends a batch when it is full.
    for i in 0..honest {
        let (window, stop, tally) = (window.clone(), stop.clone(), tally.clone());
        let src = Ipv4Addr::new(127, 1, (i / 250) as u8, (i % 250 + 1) as u8);
        let every = Duration::from_secs_f64(batch as f64 / (honest_rate / honest as f64));
        threads.push(std::thread::spawn(move || {
            let mut conn = Conn::new(src, server);
            let mut hash = protogaea_pow::c::Hasher::new(SPARK);
            let miner: [u8; 32] = random();
            let mut nonce = u64::from_le_bytes(random());
            let mut next = Instant::now();
            while !stop.load(Ordering::Relaxed) {
                let w = *window.read().expect("the lock is never poisoned");
                if !w.open {
                    std::thread::sleep(Duration::from_millis(200));
                    continue;
                }
                let mut body = Vec::with_capacity(batch * 72);
                while body.len() < batch * 72 {
                    nonce = nonce.wrapping_add(1);
                    let input =
                        spark_input(&w.world_id, w.epoch, &w.challenge, &proposal, &miner, nonce);
                    if meets(&hash.hash(&input), w.target) {
                        body.extend_from_slice(
                            &Spark {
                                proposal_id: proposal,
                                miner,
                                nonce,
                            }
                            .to_bytes(),
                        );
                    }
                }
                if *window.read().expect("the lock is never poisoned") != w {
                    continue;
                }
                let now = Instant::now();
                if next > now {
                    std::thread::sleep(next - now);
                }
                next = next.max(now) + every;
                let started = Instant::now();
                let answer = conn.request("POST", "/v0/sparks", &body);
                let ms = started.elapsed().as_secs_f64() * 1000.0;
                let mut t = tally.lock().expect("the lock is never poisoned");
                t.latency.push(ms);
                match answer {
                    Ok((200, b)) => {
                        let v: Value = serde_json::from_slice(&b).unwrap_or_default();
                        for r in v["results"].as_array().into_iter().flatten() {
                            let key = if r.get("receipt").is_some() {
                                "receipt".to_string()
                            } else {
                                r["error"].as_str().unwrap_or("?").to_string()
                            };
                            *t.honest.entry(key).or_default() += 1;
                        }
                    }
                    Ok((status, b)) => {
                        let v: Value = serde_json::from_slice(&b).unwrap_or_default();
                        let key = format!("{status} {}", v["error"].as_str().unwrap_or(""));
                        *t.honest.entry(key).or_default() += batch as u64;
                    }
                    Err(e) => *t.honest.entry(format!("io: {e}")).or_default() += batch as u64,
                }
            }
        }));
    }

    // The flood: random nonces from many addresses, spread over threads.
    let flood_threads = if flood_addrs > 0 {
        16.min(flood_addrs)
    } else {
        0
    };
    for f in 0..flood_threads {
        let (stop, tally) = (stop.clone(), tally.clone());
        let mine: Vec<usize> = (f..flood_addrs).step_by(flood_threads).collect();
        // 8 sparks a request.
        let per_request = 8usize;
        let every = Duration::from_secs_f64(per_request as f64 * flood_threads as f64 / flood_rate);
        threads.push(std::thread::spawn(move || {
            let mut conns: Vec<Conn> = mine
                .iter()
                .map(|&a| {
                    let subnet = a / per_subnet.max(1);
                    let host = a % per_subnet.max(1);
                    Conn::new(
                        Ipv4Addr::new(
                            127,
                            10 + (subnet / 250) as u8,
                            (subnet % 250) as u8,
                            (host + 1) as u8,
                        ),
                        server,
                    )
                })
                .collect();
            let mut next = Instant::now();
            let mut k = 0usize;
            while !stop.load(Ordering::Relaxed) {
                let body: Vec<u8> = (0..per_request)
                    .flat_map(|_| {
                        Spark {
                            proposal_id: proposal,
                            miner: random(),
                            nonce: u64::from_le_bytes(random()),
                        }
                        .to_bytes()
                    })
                    .collect();
                let now = Instant::now();
                if next > now {
                    std::thread::sleep(next - now);
                }
                next = next.max(now) + every;
                let c = &mut conns[k % mine.len()];
                k += 1;
                let status = match c.request("POST", "/v0/sparks", &body) {
                    Ok((s, _)) => s,
                    Err(_) => 0,
                };
                let mut t = tally.lock().expect("the lock is never poisoned");
                *t.flood.entry(status).or_default() += 1;
                t.flood_sparks += per_request as u64;
            }
        }));
    }

    // The window, the progress and the server's CPU, once a second.
    let started = Instant::now();
    let cpu0 = pid.and_then(cpu_seconds);
    let mut last_cpu = cpu0;
    let mut peak_cores = 0f64;
    let mut last_receipts = 0u64;
    while started.elapsed().as_secs_f64() < seconds {
        std::thread::sleep(Duration::from_secs(1));
        if let Ok(w) = read_window(&mut probe) {
            *window.write().expect("the lock is never poisoned") = w;
        }
        let cpu = pid.and_then(cpu_seconds);
        if let (Some(a), Some(b)) = (last_cpu, cpu) {
            peak_cores = peak_cores.max(b - a);
        }
        last_cpu = cpu;
        let t = tally.lock().expect("the lock is never poisoned");
        let receipts = t.honest.get("receipt").copied().unwrap_or(0);
        if started.elapsed().as_secs().is_multiple_of(5) {
            println!(
                "{:>4.0} s  honest {:>5} receipts (+{}/5 s)  flood {:>7} sparks  window {} {}",
                started.elapsed().as_secs_f64(),
                receipts,
                receipts - last_receipts,
                t.flood_sparks,
                window.read().expect("the lock is never poisoned").epoch,
                if window.read().expect("the lock is never poisoned").open {
                    "open"
                } else {
                    "closed"
                },
            );
            last_receipts = receipts;
        }
    }
    stop.store(true, Ordering::Relaxed);
    let elapsed = started.elapsed().as_secs_f64();
    for t in threads {
        let _ = t.join();
    }
    let mut t = tally.lock().expect("the lock is never poisoned");
    t.latency.sort_by(|a, b| a.total_cmp(b));
    let receipts = t.honest.get("receipt").copied().unwrap_or(0);
    println!("--- {elapsed:.0} s");
    println!(
        "honest: {honest} clients, {:.1} sparks accepted a second ({receipts} in all); answers {:?}",
        receipts as f64 / elapsed,
        t.honest
    );
    println!(
        "honest latency ms: p50 {:.0}, p90 {:.0}, p99 {:.0}, max {:.0} ({} requests)",
        percentile(&t.latency, 0.5),
        percentile(&t.latency, 0.9),
        percentile(&t.latency, 0.99),
        t.latency.last().copied().unwrap_or(0.0),
        t.latency.len()
    );
    if flood_addrs > 0 {
        println!(
            "flood: {flood_addrs} addresses ({per_subnet} a subnet), {:.0} sparks a second sent; requests by status {:?}",
            t.flood_sparks as f64 / elapsed,
            t.flood
        );
    }
    if let (Some(a), Some(b)) = (cpu0, pid.and_then(cpu_seconds)) {
        println!(
            "server CPU: {:.2} cores on average, {:.2} at peak (1 s)",
            (b - a) / elapsed,
            peak_cores
        );
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
