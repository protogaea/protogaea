//! Spark hash rates: one thread, then every thread, and the time to check one spark.
//!
//!   cargo run --release -p protogaea-pow --example bench [-- seconds]
//!   cargo run --release -p protogaea-pow --features c --example bench   # the reference C too

use std::time::{Duration, Instant};

use protogaea_pow::{spark_input, Params, SPARK};

/// What is measured: the Rust port, or the reference C implementation.
trait Hash {
    fn new(params: Params) -> Self;
    fn hash(&mut self, input: &[u8]) -> [u8; 32];
}

impl Hash for protogaea_pow::Hasher {
    fn new(params: Params) -> Self {
        protogaea_pow::Hasher::new(params)
    }
    fn hash(&mut self, input: &[u8]) -> [u8; 32] {
        protogaea_pow::Hasher::hash(self, input)
    }
}

#[cfg(feature = "c")]
impl Hash for protogaea_pow::c::Hasher {
    fn new(params: Params) -> Self {
        protogaea_pow::c::Hasher::new(params)
    }
    fn hash(&mut self, input: &[u8]) -> [u8; 32] {
        protogaea_pow::c::Hasher::hash(self, input)
    }
}

fn rate<H: Hash>(threads: usize, secs: f64) -> f64 {
    let stop = Duration::from_secs_f64(secs);
    let counts: Vec<u64> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let mut h = H::new(SPARK);
                    let start = Instant::now();
                    let mut n = 0u64;
                    while start.elapsed() < stop {
                        let input = spark_input(&[7; 16], 1, &[1; 32], &[2; 32], &[t as u8; 32], n);
                        std::hint::black_box(h.hash(&input));
                        n += 1;
                    }
                    n
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a bench thread"))
            .collect()
    });
    counts.iter().sum::<u64>() as f64 / secs
}

fn measure<H: Hash>(name: &str, secs: f64, cores: usize) {
    // The cost of checking one spark: a fresh hasher (its memory included) and one hash.
    let mut times: Vec<f64> = (0..50)
        .map(|i| {
            let t = Instant::now();
            let mut h = H::new(SPARK);
            std::hint::black_box(
                h.hash(&spark_input(&[7; 16], 1, &[1; 32], &[2; 32], &[3; 32], i)),
            );
            t.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    times.sort_by(f64::total_cmp);
    println!(
        "{name}: check one spark (fresh memory): p50 {:.2} ms, p95 {:.2} ms",
        times[25], times[47]
    );
    let one = rate::<H>(1, secs);
    println!(
        "{name}: 1 thread: {one:.0} H/s ({:.2} ms per hash)",
        1e3 / one
    );
    if cores >= 4 {
        println!(
            "{name}: {} threads: {:.0} H/s",
            cores / 2,
            rate::<H>(cores / 2, secs)
        );
    }
    let all = rate::<H>(cores, secs);
    println!(
        "{name}: {cores} threads: {all:.0} H/s ({:.2}x of one)",
        all / one
    );
}

fn main() {
    let secs: f64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10.0);
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    println!(
        "yespower 1.0, N = {}, r = {}, {} MiB per thread",
        SPARK.n,
        SPARK.r,
        SPARK.memory() >> 20
    );
    measure::<protogaea_pow::Hasher>("rust", secs, cores);
    #[cfg(feature = "c")]
    measure::<protogaea_pow::c::Hasher>("c", secs, cores);
}
