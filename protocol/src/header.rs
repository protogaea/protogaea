//! Signed epoch headers (spec §9, §15): what the operator commits to for each epoch, chained by
//! the previous header's hash and signed with the operator key.
//!
//! The encoding (Proposed) is fixed-width, little-endian:
//!
//! ```text
//! "PROTOGAEA/HEADER/V0" ‖ epoch u64 ‖ prev_header_hash [32] ‖ ruleset_id [32] ‖ state_root [32]
//! ‖ ledger_root [32] ‖ sth_size u64 ‖ sth_root [32] ‖ beacon [32] ‖ miracles_root [32]
//! ‖ timestamp_ms u64
//! ```
//!
//! `header_hash = BLAKE3(bytes)`, and the signature is Ed25519 over `header_hash`. The ledger root
//! commits to the price and to every wish still open or queued with its work; the miracles root
//! to the miracles given to the world in this epoch and what became of them.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::log::{leaf_hash, root};
use crate::{hash, Hash};

pub const HEADER_TAG: &[u8] = b"PROTOGAEA/HEADER/V0";
pub const LEDGER_TAG: &[u8] = b"PROTOGAEA/LEDGER/V0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub epoch: u64,
    pub prev_header_hash: Hash,
    pub ruleset_id: Hash,
    pub state_root: Hash,
    pub ledger_root: Hash,
    /// The final signed tree head of the epoch's spark log: its size and root.
    pub sth_size: u64,
    pub sth_root: Hash,
    pub beacon: Hash,
    pub miracles_root: Hash,
    /// Informational only.
    pub timestamp_ms: u64,
}

impl Header {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = HEADER_TAG.to_vec();
        out.extend_from_slice(&self.epoch.to_le_bytes());
        out.extend_from_slice(&self.prev_header_hash);
        out.extend_from_slice(&self.ruleset_id);
        out.extend_from_slice(&self.state_root);
        out.extend_from_slice(&self.ledger_root);
        out.extend_from_slice(&self.sth_size.to_le_bytes());
        out.extend_from_slice(&self.sth_root);
        out.extend_from_slice(&self.beacon);
        out.extend_from_slice(&self.miracles_root);
        out.extend_from_slice(&self.timestamp_ms.to_le_bytes());
        out
    }

    pub fn hash(&self) -> Hash {
        hash(&[&self.to_bytes()])
    }

    pub fn sign(&self, operator: &[u8; 32]) -> [u8; 64] {
        SigningKey::from_bytes(operator)
            .sign(&self.hash())
            .to_bytes()
    }

    /// Strict Ed25519 verification of the operator's signature over the header hash.
    pub fn verify(&self, operator_pubkey: &[u8; 32], signature: &[u8; 64]) -> bool {
        VerifyingKey::from_bytes(operator_pubkey).is_ok_and(|k| {
            k.verify_strict(&self.hash(), &Signature::from_bytes(signature))
                .is_ok()
        })
    }
}

/// A wish in the ledger: its id, accumulated work and status (0 open, 1 queued).
pub fn ledger_leaf(proposal_id: &Hash, work: u128, queued: bool) -> Hash {
    let mut leaf = proposal_id.to_vec();
    leaf.extend_from_slice(&work.to_le_bytes());
    leaf.push(u8::from(queued));
    leaf_hash(&leaf)
}

/// `ledger_root = BLAKE3("PROTOGAEA/LEDGER/V0" ‖ price u128 ‖ root(leaves))`, the leaves sorted by
/// proposal id.
pub fn ledger_root(price: u128, leaves: &[Hash]) -> Hash {
    hash(&[LEDGER_TAG, &price.to_le_bytes(), &root(leaves)])
}

/// A miracle of the epoch: the wish and its outcome (0 applied, 1 deferred, 2 refused for good).
pub fn miracle_leaf(proposal_id: &Hash, outcome: u8) -> Hash {
    let mut leaf = proposal_id.to_vec();
    leaf.push(outcome);
    leaf_hash(&leaf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex;
    use crate::wish::public_key;

    fn sample() -> Header {
        Header {
            epoch: 42,
            prev_header_hash: [1; 32],
            ruleset_id: [2; 32],
            state_root: [3; 32],
            ledger_root: ledger_root(2_900_000, &[ledger_leaf(&[7; 32], 123, false)]),
            sth_size: 9,
            sth_root: [4; 32],
            beacon: [5; 32],
            miracles_root: root(&[miracle_leaf(&[7; 32], 0)]),
            timestamp_ms: 1_790_000_000_000,
        }
    }

    #[test]
    fn signed_headers_verify_and_bind_every_field() {
        let h = sample();
        assert_eq!(
            h.to_bytes().len(),
            HEADER_TAG.len() + 8 + 32 * 4 + 8 + 32 * 3 + 8
        );
        let sig = h.sign(&[9; 32]);
        let op = public_key(&[9; 32]);
        assert!(h.verify(&op, &sig));
        assert!(!h.verify(&public_key(&[8; 32]), &sig));
        let mut other = h;
        other.state_root[0] ^= 1;
        assert!(!other.verify(&op, &sig), "the state root is signed");
        let mut other = h;
        other.ledger_root = ledger_root(2_900_001, &[ledger_leaf(&[7; 32], 123, false)]);
        assert!(!other.verify(&op, &sig), "the price is in the ledger root");
    }

    /// Test vector: the sample header's hash.
    #[test]
    fn header_vector() {
        assert_eq!(
            hex(&sample().hash()),
            "72d8395638a56ca439e9a663aa3dd6ae60e63ae10c3184c1f1df4c15f301bb2a"
        );
    }
}
