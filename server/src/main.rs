//! The Protogaea world server (stage B1): runs the authoritative world on a timer, keeps the
//! event log and snapshots, and serves the read API of spec §23.

mod api;
mod model;
mod store;
mod world;

use std::path::PathBuf;
use std::process::ExitCode;

use protogaea_core::Ruleset;

const USAGE: &str = "\
protogaea-server — runs a Protogaea world and serves its read API

USAGE:
  protogaea-server [--seed N] [--data DIR] [--listen ADDR] [--epoch-seconds S]
                   [--archive-every K] [--ruleset FILE] [--viewer DIR]

  --seed N           the world's seed for a new world (1); a saved world keeps its own
  --data DIR         where the world, snapshots and the database live (runs/server)
  --listen ADDR      the HTTP address (127.0.0.1:8080)
  --epoch-seconds S  one epoch every S seconds (300)
  --archive-every K  keep a snapshot every K epochs for the time machine (36)
  --ruleset FILE     a ruleset for a new world (the default ruleset)
  --viewer DIR       serve the viewer's static files from DIR at /app/

Set PROTOGAEA_PASSWORD (and PROTOGAEA_USER, by default `protogaea`) to put everything but
/health behind a password.
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == &format!("--{name}"))
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn number<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> Result<T, String> {
    value(args, name).map_or(Ok(default), |v| {
        v.parse()
            .map_err(|_| format!("--{name}: not a number: {v}"))
    })
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{USAGE}");
        return Ok(());
    }
    let rules = match value(&args, "ruleset") {
        Some(path) => {
            let text =
                std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
            serde_json::from_str::<Ruleset>(&text)
                .map_err(|e| format!("invalid ruleset {path}: {e}"))?
        }
        None => Ruleset::default(),
    };
    let opts = world::Options {
        seed: number(&args, "seed", 1)?,
        rules,
        data: PathBuf::from(value(&args, "data").unwrap_or("runs/server")),
        epoch_seconds: number(&args, "epoch-seconds", 300)?,
        archive_every: number(&args, "archive-every", 36)?,
    };
    let listen = value(&args, "listen")
        .unwrap_or("127.0.0.1:8080")
        .to_string();
    let viewer = value(&args, "viewer").map(PathBuf::from);
    let credentials = std::env::var("PROTOGAEA_PASSWORD").ok().map(|password| {
        let user = std::env::var("PROTOGAEA_USER").unwrap_or_else(|_| "protogaea".to_string());
        format!("Basic {}", base64(format!("{user}:{password}").as_bytes()))
    });
    if credentials.is_none() {
        eprintln!("note: no PROTOGAEA_PASSWORD set; the API is open to everyone who can reach it");
    }

    let (run, store, shared) = world::start(opts)?;
    {
        let shared = shared.clone();
        std::thread::Builder::new()
            .name("world".into())
            .spawn(move || {
                if let Err(e) = world::run_loop(run, store, shared) {
                    eprintln!("error: the world stopped: {e}");
                    std::process::exit(1);
                }
            })
            .map_err(|e| format!("cannot start the world thread: {e}"))?;
    }

    let mut app = api::router(shared, credentials);
    if let Some(dir) = viewer {
        app = app.nest_service("/app", tower_http::services::ServeDir::new(dir));
    }
    let app = app
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(tower_http::cors::CorsLayer::permissive());

    // A few threads are plenty for a read API, and they stay within the service's task limit.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .max_blocking_threads(16)
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(&listen)
            .await
            .map_err(|e| format!("cannot listen on {listen}: {e}"))?;
        println!("serving on http://{listen}");
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
            .map_err(|e| e.to_string())
    })
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
