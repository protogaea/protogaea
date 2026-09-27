//! A closed epoch's spark log in one blob. Wish ids and miner keys are 32 bytes each; they are
//! kept once for all epochs in the log's dictionary (`dict`), and a blob names them by number.
//! An epoch has few wishes and few miners, so each is named once in its blob; sparks come in runs from one miner for one wish (a batch); and each miner's
//! threads try nonces one after another, so a nonce is kept as the step from the last nonce of
//! its thread (its "lane"). About 2–3 bytes a spark, instead of the 72 of its wire form and the
//! ~190 of a row with its indexes.
//!
//! ```text
//! version u8 = 2
//! varint W, W × varint dictionary number of a proposal_id   in order of first use
//! varint M, M × varint dictionary number of a miner         in order of first use
//! varint N                                sparks, in log order, as runs:
//!   varint length, varint wish, varint miner
//!   length × nonce:  varint v, with L the miner's lanes so far, k = v mod (L + 1):
//!                    k = 0: nonce u64 LE follows        a new lane of the miner
//!                    k ≥ 1: lane k − 1 moves by zigzag⁻¹(v div (L + 1)) (wrapping)
//! ```
//!
//! A nonce joins the miner's lane nearest to it if the step is under 2³²; otherwise it opens a
//! lane. Version 1 (the ids and keys written out in the blob, then every spark as
//! `varint wish, varint miner, nonce u64 LE`) is still read.

use std::collections::{BTreeSet, HashMap};

use protogaea_protocol::spark::Spark;
use protogaea_protocol::Hash;

const VERSION: u8 = 2;
/// A step at least this long opens a new lane.
const FAR: u64 = 1 << 32;

fn put(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn take(b: &[u8], at: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *b.get(*at)?;
        *at += 1;
        v |= u64::from(byte & 0x7f) << shift;
        if byte < 0x80 {
            return Some(v);
        }
    }
    None
}

fn take32(b: &[u8], at: &mut usize) -> Option<Hash> {
    let h: Hash = b.get(*at..*at + 32)?.try_into().ok()?;
    *at += 32;
    Some(h)
}

fn take64(b: &[u8], at: &mut usize) -> Option<u64> {
    let v = u64::from_le_bytes(b.get(*at..*at + 8)?.try_into().ok()?);
    *at += 8;
    Some(v)
}

fn zigzag(step: i64) -> u64 {
    ((step << 1) ^ (step >> 63)) as u64
}

fn unzigzag(v: u64) -> i64 {
    (v >> 1) as i64 ^ -((v & 1) as i64)
}

/// A miner's lanes as the packer sees them: the last nonce of each, and the same sorted, to find
/// the nearest.
#[derive(Default)]
struct Lanes {
    last: Vec<u64>,
    sorted: BTreeSet<(u64, usize)>,
}

impl Lanes {
    /// Writes a nonce and moves its lane to it.
    fn put(&mut self, out: &mut Vec<u8>, nonce: u64) {
        let below = self.sorted.range(..=(nonce, usize::MAX)).next_back();
        let above = self.sorted.range((nonce, 0)..).next();
        let nearest = below
            .into_iter()
            .chain(above)
            .map(|&(v, k)| (nonce.wrapping_sub(v) as i64, k))
            .min_by_key(|(step, _)| step.unsigned_abs());
        match nearest {
            Some((step, k)) if step.unsigned_abs() < FAR => {
                // The step and the lane in one varint: under 2³³ · (lanes + 1), it fits.
                put(
                    out,
                    zigzag(step) * (self.last.len() as u64 + 1) + k as u64 + 1,
                );
                self.sorted.remove(&(self.last[k], k));
                self.last[k] = nonce;
                self.sorted.insert((nonce, k));
            }
            _ => {
                put(out, 0);
                out.extend_from_slice(&nonce.to_le_bytes());
                self.sorted.insert((nonce, self.last.len()));
                self.last.push(nonce);
            }
        }
    }
}

/// Every wish id and miner key of these sparks, once, to be numbered in the dictionary.
pub fn keys(sparks: &[Spark]) -> Vec<Hash> {
    let mut seen = std::collections::HashSet::new();
    sparks
        .iter()
        .flat_map(|s| [s.proposal_id, s.miner])
        .filter(|k| seen.insert(*k))
        .collect()
}

/// Packs sparks, in log order; `number` gives each key's number in the dictionary (all of
/// [`keys`] must have one).
pub fn pack(sparks: &[Spark], number: &HashMap<Hash, u64>) -> Vec<u8> {
    let mut wishes: HashMap<Hash, u64> = HashMap::new();
    let mut miners: HashMap<Hash, u64> = HashMap::new();
    let (mut wish_list, mut miner_list) = (Vec::new(), Vec::new());
    let mut lanes: Vec<Lanes> = Vec::new();
    let mut body = Vec::with_capacity(sparks.len() * 4);
    for run in sparks.chunk_by(|a, b| a.proposal_id == b.proposal_id && a.miner == b.miner) {
        let first = run[0];
        let w = *wishes.entry(first.proposal_id).or_insert_with(|| {
            wish_list.push(first.proposal_id);
            wish_list.len() as u64 - 1
        });
        let m = *miners.entry(first.miner).or_insert_with(|| {
            miner_list.push(first.miner);
            lanes.push(Lanes::default());
            miner_list.len() as u64 - 1
        });
        put(&mut body, run.len() as u64);
        put(&mut body, w);
        put(&mut body, m);
        for s in run {
            lanes[m as usize].put(&mut body, s.nonce);
        }
    }
    let mut out = vec![VERSION];
    for list in [&wish_list, &miner_list] {
        put(&mut out, list.len() as u64);
        list.iter().for_each(|h| put(&mut out, number[h]));
    }
    put(&mut out, sparks.len() as u64);
    out.extend_from_slice(&body);
    out
}

/// The sparks of a packed log, or `None` if it is not one; `key` reads the dictionary.
pub fn unpack(b: &[u8], key: impl Fn(u64) -> Option<Hash>) -> Option<Vec<Spark>> {
    let version = *b.first()?;
    if version != 1 && version != 2 {
        return None;
    }
    let mut at = 1;
    let list = |at: &mut usize| -> Option<Vec<Hash>> {
        let n = take(b, at)? as usize;
        (0..n)
            .map(|_| {
                if version == 1 {
                    take32(b, at)
                } else {
                    key(take(b, at)?)
                }
            })
            .collect()
    };
    let wishes = list(&mut at)?;
    let miners = list(&mut at)?;
    let n = take(b, &mut at)? as usize;
    let mut out = Vec::with_capacity(n.min(b.len()));
    if version == 1 {
        for _ in 0..n {
            let proposal_id = *wishes.get(take(b, &mut at)? as usize)?;
            let miner = *miners.get(take(b, &mut at)? as usize)?;
            let nonce = take64(b, &mut at)?;
            out.push(Spark {
                proposal_id,
                miner,
                nonce,
            });
        }
        return (at == b.len()).then_some(out);
    }
    let mut lanes: Vec<Vec<u64>> = vec![Vec::new(); miners.len()];
    while out.len() < n {
        let length = take(b, &mut at)? as usize;
        if length == 0 || out.len() + length > n {
            return None;
        }
        let proposal_id = *wishes.get(take(b, &mut at)? as usize)?;
        let m = take(b, &mut at)? as usize;
        let miner = *miners.get(m)?;
        let lanes = &mut lanes[m];
        for _ in 0..length {
            let v = take(b, &mut at)?;
            let base = lanes.len() as u64 + 1;
            let nonce = match (v % base, v / base) {
                (0, 0) => {
                    let nonce = take64(b, &mut at)?;
                    lanes.push(nonce);
                    nonce
                }
                (0, _) => return None,
                (k, step) => {
                    let lane = lanes.get_mut(k as usize - 1)?;
                    *lane = lane.wrapping_add(unzigzag(step) as u64);
                    *lane
                }
            };
            out.push(Spark {
                proposal_id,
                miner,
                nonce,
            });
        }
    }
    (at == b.len()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test's keys are 32 equal bytes: that byte is their number.
    fn numbers(sparks: &[Spark]) -> HashMap<Hash, u64> {
        keys(sparks)
            .into_iter()
            .map(|k| (k, u64::from(k[0])))
            .collect()
    }

    fn key(n: u64) -> Option<Hash> {
        Some([n as u8; 32])
    }

    /// A small deterministic generator, so the test needs no randomness.
    fn next(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    /// An epoch as miners make it: 100 miners, each on 4 threads from a random start (lanes
    /// 2⁴⁰ apart, as the spark client does), a spark every ~256 hashes, sent in batches of 16 from
    /// one miner, the batches of different miners interleaved.
    fn mined(sparks: usize) -> Vec<Spark> {
        let mut rng = 7u64;
        let mut threads: Vec<(u8, u64)> = (0..100u8)
            .flat_map(|m| {
                let base = next(&mut rng);
                (0..4u64).map(move |t| (m, base.wrapping_add(t << 40)))
            })
            .collect();
        let mut out = Vec::new();
        while out.len() < sparks {
            let m = (next(&mut rng) % 100) as usize;
            for _ in 0..16 {
                let t = &mut threads[m * 4 + (next(&mut rng) % 4) as usize];
                t.1 = t.1.wrapping_add(1 + next(&mut rng) % 512);
                out.push(Spark {
                    proposal_id: [(m % 3) as u8; 32],
                    miner: [t.0; 32],
                    nonce: t.1,
                });
            }
        }
        out.truncate(sparks);
        out
    }

    #[test]
    fn round_trip_and_size() {
        let sparks = mined(20_000);
        let b = pack(&sparks, &numbers(&sparks));
        assert_eq!(unpack(&b, key).unwrap(), sparks);
        let per_spark = b.len() as f64 / 20_000.0;
        assert!(per_spark < 3.0, "{per_spark} bytes a spark");
        println!("mined: {per_spark:.2} bytes a spark");
        // The worst case: random nonces, each spark from another miner (a run and a lane each),
        // about 12 bytes a spark, near version 1.
        let mut rng = 3u64;
        let random: Vec<Spark> = (0..5_000)
            .map(|i| Spark {
                proposal_id: [1; 32],
                miner: [(i % 50) as u8; 32],
                nonce: next(&mut rng),
            })
            .collect();
        let r = pack(&random, &numbers(&random));
        assert_eq!(unpack(&r, key).unwrap(), random);
        assert!(r.len() < 5_000 * 13, "{} bytes", r.len());
        println!("random: {:.2} bytes a spark", r.len() as f64 / 5_000.0);
        assert_eq!(
            unpack(&pack(&[], &HashMap::new()), key).unwrap(),
            Vec::<Spark>::new()
        );
        // A truncated or extended blob is refused.
        assert!(unpack(&b[..b.len() - 1], key).is_none());
        assert!(unpack(&[b.clone(), vec![0]].concat(), key).is_none());
        // Steps both ways and across the wrap.
        let edge: Vec<Spark> = [5u64, 3, u64::MAX, 1, 1 << 63, 7]
            .iter()
            .map(|&nonce| Spark {
                proposal_id: [1; 32],
                miner: [2; 32],
                nonce,
            })
            .collect();
        assert_eq!(unpack(&pack(&edge, &numbers(&edge)), key).unwrap(), edge);
    }

    /// A log packed with version 1 still reads.
    #[test]
    fn version_1_reads() {
        let mut b = vec![1u8];
        put(&mut b, 1);
        b.extend_from_slice(&[4; 32]);
        put(&mut b, 1);
        b.extend_from_slice(&[5; 32]);
        put(&mut b, 2);
        for nonce in [9u64, 3] {
            put(&mut b, 0);
            put(&mut b, 0);
            b.extend_from_slice(&nonce.to_le_bytes());
        }
        let sparks = unpack(&b, key).unwrap();
        assert_eq!(sparks.iter().map(|s| s.nonce).collect::<Vec<_>>(), [9, 3]);
        assert_eq!(sparks[1].proposal_id, [4; 32]);
    }
}
