//! Spark hash rates: one thread, then every thread, and the time to verify one spark.
//!
//!   cargo run --release -p protogaea-pow --example bench [-- seconds]

use std::time::{Duration, Instant};

use protogaea_pow::{spark_input, Hasher, SPARK};

fn rate(threads: usize, secs: f64) -> f64 {
    let stop = Duration::from_secs_f64(secs);
    let counts: Vec<u64> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                s.spawn(move || {
                    let mut h = Hasher::new(SPARK);
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

    // The cost of verifying one spark: a fresh hasher (its memory included) and one hash.
    let mut times: Vec<f64> = (0..50)
        .map(|i| {
            let t = Instant::now();
            let mut h = Hasher::new(SPARK);
            std::hint::black_box(
                h.hash(&spark_input(&[7; 16], 1, &[1; 32], &[2; 32], &[3; 32], i)),
            );
            t.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    times.sort_by(f64::total_cmp);
    println!(
        "verify one spark (fresh memory): p50 {:.2} ms, p95 {:.2} ms",
        times[25], times[47]
    );

    let one = rate(1, secs);
    println!("1 thread: {one:.0} H/s ({:.2} ms per hash)", 1e3 / one);
    let all = rate(cores, secs);
    println!(
        "{cores} threads: {all:.0} H/s ({:.0} H/s per thread, {:.2}x of one)",
        all / cores as f64,
        all / one
    );
    if cores >= 4 {
        let half = rate(cores / 2, secs);
        println!("{} threads: {half:.0} H/s", cores / 2);
    }
}
