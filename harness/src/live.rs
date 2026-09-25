//! A live world for previews: it runs in real time, saves snapshots, and serves its page
//! behind an optional password (HTTP Basic). It is a stage A preview, not the world server
//! of stage C.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use protogaea_core::{Ruleset, World};
use serde::{Deserialize, Serialize};

use crate::metrics::Tracker;
use crate::report::{LiveInfo, Recorder};
use crate::runner::Run;

const SNAPSHOT: &str = "snapshot.json";
/// The page shows the last seven world days.
const WINDOW_DAYS: u64 = 7;
/// A map frame every two world hours keeps the page near 2 MB with a full window.
const FRAME_EVERY: u64 = 24;
/// Clade counts for the Muller plot, once per world hour.
const SAMPLE_EVERY: u64 = 12;

pub struct LiveOptions {
    pub seed: u64,
    pub data: PathBuf,
    pub listen: String,
    pub epoch_seconds: u64,
    pub snapshot_every: u64,
    pub rules: Ruleset,
    /// `(user, password)`; `None` serves the page without a password.
    pub credentials: Option<(String, String)>,
}

#[derive(Deserialize)]
struct Snapshot {
    seed: u64,
    rules: Ruleset,
    world: World,
    tracker: Tracker,
    recorder: Recorder,
}

#[derive(Serialize)]
struct SnapshotRef<'a> {
    seed: u64,
    rules: &'a Ruleset,
    world: &'a World,
    tracker: &'a Tracker,
    recorder: &'a Recorder,
}

pub fn run_live(opts: LiveOptions) -> Result<(), String> {
    std::fs::create_dir_all(&opts.data)
        .map_err(|e| format!("cannot create {}: {e}", opts.data.display()))?;
    let (mut run, mut tracker, mut recorder) = match load(&opts.data)? {
        Some(s) => {
            println!("resuming seed {} at epoch {}", s.seed, s.world.epoch);
            if s.seed != opts.seed {
                eprintln!(
                    "note: the saved world keeps its seed {} and ruleset",
                    s.seed
                );
            }
            (Run::resume(s.seed, s.rules, s.world), s.tracker, s.recorder)
        }
        None => {
            println!("starting seed {} from genesis", opts.seed);
            let run = Run::new(opts.seed, opts.rules.clone());
            let tracker = Tracker::new(&run.world, &run.rules, &run.plan);
            let per_day = u64::from(run.rules.epochs_per_day);
            let mut recorder = Recorder::rolling(FRAME_EVERY, SAMPLE_EVERY, WINDOW_DAYS * per_day);
            recorder.observe(&run.world);
            (run, tracker, recorder)
        }
    };
    let window = (WINDOW_DAYS * u64::from(run.rules.epochs_per_day)) as usize;

    let page = Arc::new(Mutex::new(render(
        &run,
        &tracker,
        &recorder,
        opts.epoch_seconds,
    )?));
    let listener = TcpListener::bind(&opts.listen)
        .map_err(|e| format!("cannot listen on {}: {e}", opts.listen))?;
    let expected = opts.credentials.as_ref().map(|(user, password)| {
        format!("Basic {}", base64(format!("{user}:{password}").as_bytes()))
    });
    if expected.is_none() {
        eprintln!(
            "warning: no PROTOGAEA_USER/PROTOGAEA_PASSWORD set; the page is open to everyone"
        );
    }
    {
        let page = Arc::clone(&page);
        std::thread::spawn(move || serve(listener, page, expected));
    }
    println!(
        "serving on http://{} — a new epoch every {} s",
        opts.listen, opts.epoch_seconds
    );

    let mut next = Instant::now();
    loop {
        // World time is logical: after a delay the world does not catch up (spec §20).
        next += Duration::from_secs(opts.epoch_seconds);
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
        if !run.world.finished(&run.rules) {
            let report = run.step();
            tracker.record(&run.world, &report, &run.rules);
            tracker.trim(window);
            recorder.observe(&run.world);
            let html = render(&run, &tracker, &recorder, opts.epoch_seconds)?;
            *page.lock().expect("the page lock is never poisoned") = html;
            let epoch = run.world.epoch;
            if epoch % opts.snapshot_every.max(1) == 0 || run.world.finished(&run.rules) {
                save(&opts.data, &run, &tracker, &recorder)?;
                println!(
                    "epoch {epoch}: population {}, snapshot saved",
                    run.world.organisms.len()
                );
            }
        }
    }
}

fn render(
    run: &Run,
    tracker: &Tracker,
    recorder: &Recorder,
    epoch_seconds: u64,
) -> Result<String, String> {
    let summary = tracker.summary(run.seed, &run.rules);
    let checks = summary.checks();
    let live = LiveInfo {
        epoch: run.world.epoch,
        epoch_seconds,
    };
    recorder.render_html(run, &tracker.series, &summary, &checks, Some(live))
}

fn load(dir: &Path) -> Result<Option<Snapshot>, String> {
    let path = dir.join(SNAPSHOT);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("invalid snapshot {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

/// Writes the snapshot atomically: to a temporary file first, then renames it.
fn save(dir: &Path, run: &Run, tracker: &Tracker, recorder: &Recorder) -> Result<(), String> {
    let snapshot = SnapshotRef {
        seed: run.seed,
        rules: &run.rules,
        world: &run.world,
        tracker,
        recorder,
    };
    let json = serde_json::to_vec(&snapshot).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!("{SNAPSHOT}.tmp"));
    std::fs::write(&tmp, json).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, dir.join(SNAPSHOT))
        .map_err(|e| format!("cannot replace the snapshot: {e}"))
}

fn serve(listener: TcpListener, page: Arc<Mutex<String>>, expected: Option<String>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            // For example, out of file descriptors: back off instead of spinning.
            std::thread::sleep(Duration::from_millis(100));
            continue;
        };
        let page = Arc::clone(&page);
        let expected = expected.clone();
        // If the system refuses a thread (a task limit), the connection is simply dropped.
        let _ = std::thread::Builder::new().spawn(move || {
            let _ = handle(stream, &page, expected.as_deref());
        });
    }
}

/// A minimal HTTP/1.1 responder: `GET /` (the page, behind the password), `GET /health`.
fn handle(
    mut stream: TcpStream,
    page: &Mutex<String>,
    expected: Option<&str>,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.by_ref().take(8192).read_line(&mut request_line)?;
    let mut authorized = expected.is_none();
    let mut header_bytes = 0;
    loop {
        let mut line = String::new();
        let n = reader.by_ref().take(8192).read_line(&mut line)?;
        header_bytes += n;
        if n == 0 || line.trim().is_empty() || header_bytes > 16 * 1024 {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("authorization") && Some(value.trim()) == expected {
                authorized = true;
            }
        }
    }
    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let (status, content_type, body, extra) = match path {
        "/health" => ("200 OK", "text/plain; charset=utf-8", "ok".to_string(), ""),
        _ if !authorized => (
            "401 Unauthorized",
            "text/plain; charset=utf-8",
            "authorization required".to_string(),
            "WWW-Authenticate: Basic realm=\"Protogaea\", charset=\"UTF-8\"\r\n",
        ),
        "/" | "/index.html" => (
            "200 OK",
            "text/html; charset=utf-8",
            page.lock()
                .expect("the page lock is never poisoned")
                .clone(),
            "",
        ),
        _ => (
            "404 Not Found",
            "text/plain; charset=utf-8",
            "not found".to_string(),
            "",
        ),
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n{extra}\r\n",
        body.len()
    )?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

/// Standard base64 with padding, for the `Authorization: Basic` header.
fn base64(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for k in 0..4 {
            if k <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * k)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use protogaea_core::{Ruleset, World};

    use super::base64;
    use crate::runner::Run;

    #[test]
    fn a_resumed_world_matches_an_uninterrupted_one() {
        let rules = Ruleset::default();
        let mut a = Run::new(7, rules.clone());
        for _ in 0..5 {
            a.step();
        }
        let json = serde_json::to_string(&a.world).expect("the world serializes");
        let world: World = serde_json::from_str(&json).expect("the world deserializes");
        assert_eq!(world, a.world);
        let mut b = Run::resume(7, rules, world);
        for _ in 0..5 {
            a.step();
            b.step();
        }
        assert_eq!(a.state_root(), b.state_root());
    }

    #[test]
    fn base64_matches_rfc_4648() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(input.as_bytes()), expected);
        }
    }
}
