//! Sparks (protocol §5): the epoch challenge, the spark and its batch encoding, `spark_id`, the
//! PoW check and the epoch target.

use protogaea_pow::{meets, spark_input, weight, Hasher};

use crate::{hash, Hash};

pub const CHALLENGE_TAG: &[u8] = b"PROTOGAEA/CHALLENGE/V0";
pub const SPARK_ID_TAG: &[u8] = b"PROTOGAEA/SPARK_ID/V0";
/// A spark as submitted: proposal_id ‖ miner_pubkey ‖ nonce (u64 LE).
pub const SPARK_BYTES: usize = 72;
/// Sparks per submission at most.
pub const MAX_BATCH: usize = 64;
/// The reference number of accepted sparks per epoch across the network (spec §18).
pub const TARGET_SPARKS: u64 = 20_000;

/// `challenge_E = BLAKE3("PROTOGAEA/CHALLENGE/V0" ‖ world_id ‖ E ‖ header_hash_{E−1})`.
pub fn challenge(world_id: &[u8; 16], epoch: u64, prev_header_hash: &Hash) -> Hash {
    hash(&[
        CHALLENGE_TAG,
        world_id,
        &epoch.to_le_bytes(),
        prev_header_hash,
    ])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Spark {
    pub proposal_id: Hash,
    pub miner: [u8; 32],
    pub nonce: u64,
}

impl Spark {
    pub fn to_bytes(&self) -> [u8; SPARK_BYTES] {
        let mut out = [0u8; SPARK_BYTES];
        out[..32].copy_from_slice(&self.proposal_id);
        out[32..64].copy_from_slice(&self.miner);
        out[64..].copy_from_slice(&self.nonce.to_le_bytes());
        out
    }

    pub fn from_bytes(b: &[u8; SPARK_BYTES]) -> Spark {
        Spark {
            proposal_id: b[..32].try_into().expect("32 bytes"),
            miner: b[32..64].try_into().expect("32 bytes"),
            nonce: u64::from_le_bytes(b[64..].try_into().expect("8 bytes")),
        }
    }

    /// The leaf of the spark log (Proposed): epoch ‖ proposal_id ‖ miner_pubkey ‖ nonce, 80 bytes.
    pub fn leaf(&self, epoch: u64) -> [u8; 80] {
        let mut out = [0u8; 80];
        out[..8].copy_from_slice(&epoch.to_le_bytes());
        out[8..].copy_from_slice(&self.to_bytes());
        out
    }

    /// `spark_id = BLAKE3("PROTOGAEA/SPARK_ID/V0" ‖ epoch ‖ proposal_id ‖ miner_pubkey ‖ nonce)`.
    pub fn id(&self, epoch: u64) -> Hash {
        hash(&[SPARK_ID_TAG, &self.leaf(epoch)])
    }

    /// The PoW value: the first 8 bytes of yespower of the spark input, as it is compared with
    /// the target.
    pub fn pow(
        &self,
        hasher: &mut Hasher,
        world_id: &[u8; 16],
        epoch: u64,
        challenge: &Hash,
    ) -> [u8; 32] {
        hasher.hash(&spark_input(
            world_id,
            epoch,
            challenge,
            &self.proposal_id,
            &self.miner,
            self.nonce,
        ))
    }

    /// Checks the PoW against the epoch target; returns the spark's weight in work units.
    pub fn check(
        &self,
        hasher: &mut Hasher,
        world_id: &[u8; 16],
        epoch: u64,
        challenge: &Hash,
        target: u64,
    ) -> Option<u64> {
        meets(&self.pow(hasher, world_id, epoch, challenge), target).then(|| weight(target))
    }
}

/// Splits a submission into sparks: a whole number of 72-byte sparks, 1 to 64 of them.
pub fn parse_batch(bytes: &[u8]) -> Option<Vec<Spark>> {
    if bytes.is_empty()
        || !bytes.len().is_multiple_of(SPARK_BYTES)
        || bytes.len() / SPARK_BYTES > MAX_BATCH
    {
        return None;
    }
    Some(
        bytes
            .as_chunks::<SPARK_BYTES>()
            .0
            .iter()
            .map(Spark::from_bytes)
            .collect(),
    )
}

/// The next epoch's target (protocol §5.7): toward [`TARGET_SPARKS`] accepted sparks, by at most
/// a quarter either way. Integer arithmetic, rounding down (Proposed); never below 1.
pub fn next_target(target: u64, accepted: u64) -> u64 {
    let t = u128::from(target);
    let want = t * u128::from(TARGET_SPARKS) / u128::from(accepted.max(1));
    let (low, high) = (t * 3 / 4, t * 5 / 4);
    want.clamp(low, high).clamp(1, u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex;
    use protogaea_pow::SPARK;

    #[test]
    fn encodings() {
        let s = Spark {
            proposal_id: [3; 32],
            miner: [4; 32],
            nonce: 0x0102_0304,
        };
        assert_eq!(Spark::from_bytes(&s.to_bytes()), s);
        let two = [s.to_bytes(), s.to_bytes()].concat();
        assert_eq!(parse_batch(&two), Some(vec![s, s]));
        assert_eq!(parse_batch(&two[1..]), None);
        assert_eq!(parse_batch(&[]), None);
        assert_eq!(parse_batch(&vec![0; SPARK_BYTES * 65]), None);
        assert_eq!(&s.leaf(9)[..8], &9u64.to_le_bytes());
        assert_ne!(s.id(9), s.id(10), "the epoch is part of the identifier");
    }

    /// Test vector: a challenge, and a spark found by search under an easy target.
    #[test]
    fn challenge_and_spark_vector() {
        let c = challenge(&[1; 16], 5, &[2; 32]);
        assert_eq!(
            hex(&c),
            "8535a98fe72d5348cc9b9c9d649fea0deff63ae6fa08ea3359a320d5be3973ab"
        );
        let target = u64::MAX / 16;
        let mut h = Hasher::new(SPARK);
        let mut s = Spark {
            proposal_id: [3; 32],
            miner: [4; 32],
            nonce: 0,
        };
        while s.check(&mut h, &[1; 16], 5, &c, target).is_none() {
            s.nonce += 1;
        }
        assert_eq!(s.nonce.to_string(), "21");
        assert_eq!(s.check(&mut h, &[1; 16], 5, &c, target), Some(16));
        // Its PoW value, below the target in its first 8 bytes.
        assert_eq!(
            hex(&s.pow(&mut h, &[1; 16], 5, &c)),
            "0afdf9c8578302e5bce89afcd00c1ce49409b3b34f4c24ba99765025143681c7"
        );
    }

    #[test]
    fn target_moves_by_at_most_a_quarter() {
        assert_eq!(next_target(1000, TARGET_SPARKS), 1000);
        assert_eq!(next_target(1000, TARGET_SPARKS * 10), 750);
        assert_eq!(next_target(1000, 0), 1250);
        assert_eq!(next_target(1000, TARGET_SPARKS * 11 / 10), 909);
        assert_eq!(next_target(u64::MAX, 0), u64::MAX);
        assert_eq!(next_target(1, u64::MAX), 1);
    }
}
