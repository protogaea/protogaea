//! Signed tree heads and receipts (protocol §6).
//!
//! An STH commits the operator to the spark log of an epoch at some size. Its signed payload
//! (Proposed) is `"PROTOGAEA/STH/V0" ‖ epoch u64 ‖ tree_size u64 ‖ root [32] ‖ timestamp_ms u64`,
//! signed with the operator key (Ed25519, strict verification). A receipt ties one spark to an
//! STH by an inclusion proof, so a naturalist can later show that the operator confirmed it.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::log::{leaf_hash, verify_inclusion};
use crate::spark::Spark;
use crate::Hash;

pub const STH_TAG: &[u8] = b"PROTOGAEA/STH/V0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sth {
    pub epoch: u64,
    pub tree_size: u64,
    pub root: Hash,
    /// Informational only (spec §18): when the operator signed it, in Unix milliseconds.
    pub timestamp_ms: u64,
    pub signature: [u8; 64],
}

fn payload(epoch: u64, tree_size: u64, root: &Hash, timestamp_ms: u64) -> Vec<u8> {
    let mut out = STH_TAG.to_vec();
    out.extend_from_slice(&epoch.to_le_bytes());
    out.extend_from_slice(&tree_size.to_le_bytes());
    out.extend_from_slice(root);
    out.extend_from_slice(&timestamp_ms.to_le_bytes());
    out
}

impl Sth {
    pub fn sign(
        operator: &[u8; 32],
        epoch: u64,
        tree_size: u64,
        root: Hash,
        timestamp_ms: u64,
    ) -> Sth {
        let signature = SigningKey::from_bytes(operator)
            .sign(&payload(epoch, tree_size, &root, timestamp_ms))
            .to_bytes();
        Sth {
            epoch,
            tree_size,
            root,
            timestamp_ms,
            signature,
        }
    }

    pub fn verify(&self, operator_pubkey: &[u8; 32]) -> bool {
        let Ok(key) = VerifyingKey::from_bytes(operator_pubkey) else {
            return false;
        };
        key.verify_strict(
            &payload(self.epoch, self.tree_size, &self.root, self.timestamp_ms),
            &Signature::from_bytes(&self.signature),
        )
        .is_ok()
    }
}

/// What a naturalist keeps for each accepted spark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub spark: Spark,
    pub leaf_index: u64,
    pub sth: Sth,
    pub path: Vec<Hash>,
}

impl Receipt {
    /// The STH is the operator's, it is of the spark's epoch, and the spark is in its tree.
    pub fn verify(&self, operator_pubkey: &[u8; 32]) -> bool {
        self.sth.verify(operator_pubkey)
            && verify_inclusion(
                &leaf_hash(&self.spark.leaf(self.sth.epoch)),
                self.leaf_index,
                self.sth.tree_size,
                &self.path,
                &self.sth.root,
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::{inclusion_proof, root};
    use crate::wish::public_key;

    #[test]
    fn receipts_verify_and_forgeries_do_not() {
        let operator = [9u8; 32];
        let pubkey = public_key(&operator);
        let sparks: Vec<Spark> = (0..5)
            .map(|n| Spark {
                proposal_id: [1; 32],
                miner: [2; 32],
                nonce: n,
            })
            .collect();
        let leaves: Vec<Hash> = sparks.iter().map(|s| leaf_hash(&s.leaf(40))).collect();
        let sth = Sth::sign(&operator, 40, 5, root(&leaves), 1_790_000_000_000);
        assert!(sth.verify(&pubkey));
        let receipt = Receipt {
            spark: sparks[3],
            leaf_index: 3,
            sth,
            path: inclusion_proof(&leaves, 3),
        };
        assert!(receipt.verify(&pubkey));

        let mut forged = sth;
        forged.tree_size = 6;
        assert!(!forged.verify(&pubkey), "the size is signed");
        assert!(!sth.verify(&public_key(&[8; 32])), "another operator");
        let wrong_spark = Receipt {
            spark: sparks[2],
            ..receipt.clone()
        };
        assert!(!wrong_spark.verify(&pubkey));
        let wrong_epoch = Receipt {
            sth: Sth::sign(&operator, 41, 5, sth.root, 0),
            ..receipt
        };
        assert!(
            !wrong_epoch.verify(&pubkey),
            "the leaf commits to its epoch"
        );
    }
}
