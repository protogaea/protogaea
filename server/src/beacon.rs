//! The drand beacon for the world loop (spec §20): the round of an epoch is fixed when its window
//! closes (the first round at least 10 s later), and the loop waits for it. There is no fallback
//! seed: while drand cannot be reached, the epoch waits (spec §20, "the operator never picks a
//! convenient seed"). Every round's BLS signature is checked against the network key before use.

use std::time::Duration;

use protogaea_protocol::beacon::{round_time, verify, QUICKNET_HASH};
use protogaea_protocol::Hash;

/// Mirrors of the drand HTTP API, fastest first from the test server.
const MIRRORS: [&str; 4] = [
    "https://drand.cloudflare.com",
    "https://api.drand.sh",
    "https://api2.drand.sh",
    "https://api3.drand.sh",
];

fn fetch(mirror: &str, round: u64) -> Result<Vec<u8>, String> {
    let url = format!("{mirror}/{QUICKNET_HASH}/public/{round}");
    let mut res = ureq::get(&url)
        .config()
        .timeout_global(Some(Duration::from_secs(5)))
        .build()
        .call()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = res.body_mut().read_json().map_err(|e| e.to_string())?;
    if v["round"].as_u64() != Some(round) {
        return Err("another round".into());
    }
    let sig = v["signature"].as_str().ok_or("no signature")?;
    (0..sig.len() / 2)
        .map(|i| u8::from_str_radix(&sig[2 * i..2 * i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn now_s() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// Waits for a quicknet round and returns its verified randomness.
pub fn wait_for(round: u64) -> Hash {
    let due = round_time(round) as f64 + 0.3;
    let now = now_s();
    if due > now {
        std::thread::sleep(Duration::from_secs_f64(due - now));
    }
    let mut failures = 0u32;
    loop {
        for mirror in MIRRORS {
            match fetch(mirror, round) {
                Ok(sig) => match verify(round, &sig) {
                    Some(r) => {
                        if failures > 0 {
                            println!("drand round {round} after {failures} failed tries");
                        }
                        return r;
                    }
                    None => eprintln!("warning: drand round {round} from {mirror} does not verify"),
                },
                Err(e) => {
                    failures += 1;
                    if failures.is_power_of_two() {
                        eprintln!("warning: drand round {round} from {mirror}: {e}");
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
