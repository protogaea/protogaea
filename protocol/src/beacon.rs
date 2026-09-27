//! The randomness beacon (spec §20, protocol §8): drand's quicknet, a round every 3 seconds, each
//! a BLS signature on BLS12-381 by the drand network's threshold key over the round number. The
//! round of an epoch is fixed by a rule before its window closes, and its value is unknown to
//! anyone until the round comes, so the operator cannot pick a convenient seed.

use drand_verify::{G2PubkeyRfc, Pubkey};
use sha2::{Digest, Sha256};

use crate::Hash;

/// The quicknet chain: its hash, the network's public key, genesis time and period.
pub const QUICKNET_HASH: &str = "52db9ba70e0cc0f6eaf7803dd07447a1f5477735fd3f661792ba94600c84e971";
pub const QUICKNET_PUBKEY: &str = "83cf0f2896adee7eb8b5f01fcad3912212c437e0073e911fb90022d3e760183c8c4b450b6a0a6c3ac6a5776a2d1064510d1fec758c921cc22b0e17e63aaf4bcb5ed66304de9cf809bd274ca73bab4af5a6e9c76a4bc09e76eae8991ef5ece45a";
pub const GENESIS: u64 = 1_692_803_367;
pub const PERIOD: u64 = 3;
/// The round of an epoch is the first one at least this many seconds after its window closes.
pub const DELAY: u64 = 10;

/// The time of a round, in Unix seconds.
pub fn round_time(round: u64) -> u64 {
    GENESIS + (round.max(1) - 1) * PERIOD
}

/// The first round whose time is at least `unix_s`.
pub fn round_at(unix_s: u64) -> u64 {
    if unix_s <= GENESIS {
        return 1;
    }
    (unix_s - GENESIS).div_ceil(PERIOD) + 1
}

/// The round of an epoch whose window closed at `close_ms` (Unix milliseconds): the first round
/// no earlier than the close plus [`DELAY`] seconds.
pub fn round_for_close(close_ms: u64) -> u64 {
    round_at(close_ms.div_ceil(1000) + DELAY)
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

/// Checks a quicknet round's signature against the network key; returns the round's randomness,
/// `SHA-256(signature)`, when it holds.
pub fn verify(round: u64, signature: &[u8]) -> Option<Hash> {
    let key: [u8; 96] = unhex(QUICKNET_PUBKEY).try_into().expect("96 bytes");
    let pk = G2PubkeyRfc::from_fixed(key).ok()?;
    pk.verify(round, &[], signature)
        .ok()
        .filter(|ok| *ok)
        .map(|_| Sha256::digest(signature).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex;

    /// A real quicknet round, as `https://api.drand.sh/<chain>/public/1000000` serves it.
    #[test]
    fn a_real_round_verifies() {
        let sig = unhex("83ad29e4c409f9470fc2ef02f90214df49e02b441a1a241a82d622d9f608ef98fd8b11a029f1bee9d9e83b45088abe72");
        let r = verify(1_000_000, &sig).expect("a valid round");
        assert_eq!(
            hex(&r),
            "b22aad4794f7451896f7a371aa46106fd84d919f3f569acd5b2fddf1d1440af3"
        );
        assert_eq!(
            verify(1_000_001, &sig),
            None,
            "the signature is for its round only"
        );
        let mut bad = sig.clone();
        bad[10] ^= 1;
        assert_eq!(verify(1_000_000, &bad), None);
    }

    #[test]
    fn rounds_and_times() {
        assert_eq!(round_time(1), GENESIS);
        assert_eq!(round_at(GENESIS), 1);
        assert_eq!(round_at(GENESIS + 1), 2);
        assert_eq!(round_at(GENESIS + 3), 2);
        assert_eq!(round_at(GENESIS + 4), 3);
        for r in [2, 10, 1_000_000] {
            assert_eq!(round_at(round_time(r)), r);
        }
        let close = 1_790_000_000_500;
        let r = round_for_close(close);
        assert!(round_time(r) * 1000 >= close + DELAY * 1000);
        assert!(round_time(r - 1) * 1000 < close + DELAY * 1000);
    }
}
