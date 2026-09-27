//! A closed epoch's spark log in one blob. An epoch has few wishes and few miners, so each is
//! written once and a spark keeps only their numbers and its nonce: about 10 bytes a spark
//! instead of the 72 of its wire form (and the ~190 of a row with its indexes).
//!
//! ```text
//! version u8 = 1
//! varint W, W × proposal_id (32 bytes)    in order of first use
//! varint M, M × miner (32 bytes)          in order of first use
//! varint N, N × (varint wish, varint miner, nonce u64 LE)   in log order
//! ```

use std::collections::HashMap;

use protogaea_protocol::spark::Spark;
use protogaea_protocol::Hash;

const VERSION: u8 = 1;

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

/// Packs sparks, in log order.
pub fn pack(sparks: &[Spark]) -> Vec<u8> {
    let mut wishes: HashMap<Hash, u64> = HashMap::new();
    let mut miners: HashMap<Hash, u64> = HashMap::new();
    let (mut wish_list, mut miner_list) = (Vec::new(), Vec::new());
    let mut body = Vec::with_capacity(sparks.len() * 11);
    for s in sparks {
        let w = *wishes.entry(s.proposal_id).or_insert_with(|| {
            wish_list.push(s.proposal_id);
            wish_list.len() as u64 - 1
        });
        let m = *miners.entry(s.miner).or_insert_with(|| {
            miner_list.push(s.miner);
            miner_list.len() as u64 - 1
        });
        put(&mut body, w);
        put(&mut body, m);
        body.extend_from_slice(&s.nonce.to_le_bytes());
    }
    let mut out = vec![VERSION];
    put(&mut out, wish_list.len() as u64);
    wish_list.iter().for_each(|h| out.extend_from_slice(h));
    put(&mut out, miner_list.len() as u64);
    miner_list.iter().for_each(|h| out.extend_from_slice(h));
    put(&mut out, sparks.len() as u64);
    out.extend_from_slice(&body);
    out
}

/// The sparks of a packed log, or `None` if it is not one.
pub fn unpack(b: &[u8]) -> Option<Vec<Spark>> {
    if b.first() != Some(&VERSION) {
        return None;
    }
    let mut at = 1;
    let list = |at: &mut usize| -> Option<Vec<Hash>> {
        let n = take(b, at)? as usize;
        (0..n).map(|_| take32(b, at)).collect()
    };
    let wishes = list(&mut at)?;
    let miners = list(&mut at)?;
    let n = take(b, &mut at)? as usize;
    let mut out = Vec::with_capacity(n.min(b.len()));
    for _ in 0..n {
        let proposal_id = *wishes.get(take(b, &mut at)? as usize)?;
        let miner = *miners.get(take(b, &mut at)? as usize)?;
        let nonce = u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?);
        at += 8;
        out.push(Spark {
            proposal_id,
            miner,
            nonce,
        });
    }
    (at == b.len()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_size() {
        // 20,000 sparks for 3 wishes from 200 miners, as an epoch at the target might hold.
        let sparks: Vec<Spark> = (0..20_000u64)
            .map(|i| Spark {
                proposal_id: [(i % 3) as u8; 32],
                miner: [(i % 200) as u8; 32],
                nonce: i.wrapping_mul(0x9e37_79b9_7f4a_7c15),
            })
            .collect();
        let b = pack(&sparks);
        assert_eq!(unpack(&b).unwrap(), sparks);
        assert!(b.len() < 20_000 * 11, "{} bytes", b.len());
        assert_eq!(unpack(&pack(&[])).unwrap(), Vec::<Spark>::new());
        // A truncated or extended blob is refused.
        assert!(unpack(&b[..b.len() - 1]).is_none());
        assert!(unpack(&[b.clone(), vec![0]].concat()).is_none());
        let mut big = vec![0u8; 0];
        put(&mut big, 300);
        let mut at = 0;
        assert_eq!(take(&big, &mut at), Some(300));
    }
}
