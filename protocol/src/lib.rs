//! The spark protocol v0 ([docs/protocol.md](../../docs/protocol.md), spec Part III): what
//! naturalists send, what the operator commits to, and what anyone can check.
//!
//! - [`wish`]: the canonical bytes of a wish, its `proposal_id` and its Ed25519 signature;
//! - [`spark`]: the epoch challenge, the spark and its batch encoding, `spark_id`, the PoW check
//!   and the epoch target;
//! - [`log`]: the spark log, a Merkle tree after RFC 9162 (RFC 6962) with BLAKE3, its inclusion
//!   and consistency proofs;
//! - [`sth`]: signed tree heads and receipts;
//! - [`header`]: signed epoch headers with the ledger and miracles roots.
//!
//! Everything here is deterministic and builds for WebAssembly, so the server, the spark clients
//! and independent watchers share one implementation.

pub mod header;
pub mod log;
pub mod spark;
pub mod sth;
pub mod wish;

pub type Hash = [u8; 32];

/// BLAKE3 of the concatenation of `parts`.
pub fn hash(parts: &[&[u8]]) -> Hash {
    let mut h = blake3::Hasher::new();
    for p in parts {
        h.update(p);
    }
    *h.finalize().as_bytes()
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
