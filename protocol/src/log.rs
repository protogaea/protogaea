//! The spark log (protocol §6): one Merkle tree per epoch, after RFC 9162 §2.1 (the successor of
//! RFC 6962) with BLAKE3: leaf hash `H(0x00 ‖ leaf)`, node hash `H(0x01 ‖ left ‖ right)`, and the
//! root of an empty tree `H("")`. Inclusion proofs show that a spark is in a tree; consistency
//! proofs show that a later tree extends an earlier one, so a spark once confirmed cannot be
//! dropped unseen.

use crate::{hash, Hash};

pub fn leaf_hash(leaf: &[u8]) -> Hash {
    hash(&[&[0], leaf])
}

fn node(left: &Hash, right: &Hash) -> Hash {
    hash(&[&[1], left, right])
}

/// The largest power of two smaller than `n` (n ≥ 2).
fn split(n: usize) -> usize {
    1 << (usize::BITS - 1 - (n - 1).leading_zeros())
}

/// The root of a tree over leaf hashes.
pub fn root(leaves: &[Hash]) -> Hash {
    match leaves.len() {
        0 => hash(&[]),
        1 => leaves[0],
        n => {
            let k = split(n);
            node(&root(&leaves[..k]), &root(&leaves[k..]))
        }
    }
}

/// The audit path of leaf `m` (RFC 9162 §2.1.3.1).
pub fn inclusion_proof(leaves: &[Hash], m: usize) -> Vec<Hash> {
    assert!(m < leaves.len(), "no such leaf");
    let n = leaves.len();
    if n == 1 {
        return Vec::new();
    }
    let k = split(n);
    if m < k {
        let mut p = inclusion_proof(&leaves[..k], m);
        p.push(root(&leaves[k..]));
        p
    } else {
        let mut p = inclusion_proof(&leaves[k..], m - k);
        p.push(root(&leaves[..k]));
        p
    }
}

/// Checks an audit path (RFC 9162 §2.1.3.2).
pub fn verify_inclusion(leaf: &Hash, index: u64, size: u64, path: &[Hash], root: &Hash) -> bool {
    if index >= size {
        return false;
    }
    let (mut f_n, mut s_n) = (index, size - 1);
    let mut r = *leaf;
    for p in path {
        if s_n == 0 {
            return false;
        }
        if f_n & 1 == 1 || f_n == s_n {
            r = node(p, &r);
            if f_n & 1 == 0 {
                while f_n & 1 == 0 && f_n != 0 {
                    f_n >>= 1;
                    s_n >>= 1;
                }
            }
        } else {
            r = node(&r, p);
        }
        f_n >>= 1;
        s_n >>= 1;
    }
    s_n == 0 && r == *root
}

/// A proof that the tree of the first `m` leaves is a prefix of the tree of all of them
/// (RFC 9162 §2.1.4.1).
pub fn consistency_proof(leaves: &[Hash], m: usize) -> Vec<Hash> {
    assert!(0 < m && m <= leaves.len(), "no such tree");
    fn subproof(m: usize, d: &[Hash], complete: bool) -> Vec<Hash> {
        let n = d.len();
        if m == n {
            return if complete { Vec::new() } else { vec![root(d)] };
        }
        let k = split(n);
        if m <= k {
            let mut p = subproof(m, &d[..k], complete);
            p.push(root(&d[k..]));
            p
        } else {
            let mut p = subproof(m - k, &d[k..], false);
            p.push(root(&d[..k]));
            p
        }
    }
    subproof(m, leaves, true)
}

/// Checks a consistency proof between an earlier tree (`first`, `first_root`) and a later one
/// (RFC 9162 §2.1.4.2).
pub fn verify_consistency(
    first: u64,
    second: u64,
    first_root: &Hash,
    second_root: &Hash,
    proof: &[Hash],
) -> bool {
    if first == 0 || first > second {
        return false;
    }
    if first == second {
        return proof.is_empty() && first_root == second_root;
    }
    let mut path: Vec<Hash> = proof.to_vec();
    if first.is_power_of_two() {
        path.insert(0, *first_root);
    }
    let Some((&start, rest)) = path.split_first() else {
        return false;
    };
    let (mut f_n, mut s_n) = (first - 1, second - 1);
    while f_n & 1 == 1 {
        f_n >>= 1;
        s_n >>= 1;
    }
    let (mut fr, mut sr) = (start, start);
    for c in rest {
        if s_n == 0 {
            return false;
        }
        if f_n & 1 == 1 || f_n == s_n {
            fr = node(c, &fr);
            sr = node(c, &sr);
            if f_n & 1 == 0 {
                while f_n & 1 == 0 && f_n != 0 {
                    f_n >>= 1;
                    s_n >>= 1;
                }
            }
        } else {
            sr = node(&sr, c);
        }
        f_n >>= 1;
        s_n >>= 1;
    }
    fr == *first_root && sr == *second_root && s_n == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex;

    fn leaves(n: usize) -> Vec<Hash> {
        (0..n as u32).map(|i| leaf_hash(&i.to_le_bytes())).collect()
    }

    #[test]
    fn every_inclusion_proof_holds_and_tampering_fails() {
        for n in 1..=33 {
            let d = leaves(n);
            let r = root(&d);
            for m in 0..n {
                let p = inclusion_proof(&d, m);
                assert!(
                    verify_inclusion(&d[m], m as u64, n as u64, &p, &r),
                    "n {n} m {m}"
                );
                // (A path does not bind the size by itself: the STH signs size and root together.)
                assert!(!verify_inclusion(
                    &d[m],
                    m as u64,
                    n as u64,
                    &p,
                    &leaf_hash(b"other root")
                ));
                if n > 1 {
                    assert!(!verify_inclusion(
                        &d[(m + 1) % n],
                        m as u64,
                        n as u64,
                        &p,
                        &r
                    ));
                    let mut bad = p.clone();
                    bad[0][0] ^= 1;
                    assert!(!verify_inclusion(&d[m], m as u64, n as u64, &bad, &r));
                }
            }
        }
    }

    #[test]
    fn every_consistency_proof_holds_and_tampering_fails() {
        for n in 1..=33 {
            let d = leaves(n);
            let r = root(&d);
            for m in 1..=n {
                let p = consistency_proof(&d, m);
                let rm = root(&d[..m]);
                assert!(
                    verify_consistency(m as u64, n as u64, &rm, &r, &p),
                    "m {m} n {n}"
                );
                if m < n {
                    // A first tree that is not a prefix: its last leaf changed.
                    let mut other = d[..m].to_vec();
                    other[m - 1] = leaf_hash(b"dropped and replaced");
                    assert!(!verify_consistency(
                        m as u64,
                        n as u64,
                        &root(&other),
                        &r,
                        &p
                    ));
                    assert!(!verify_consistency(
                        m as u64,
                        n as u64,
                        &rm,
                        &leaf_hash(b"x"),
                        &p
                    ));
                }
            }
        }
    }

    /// Test vector: the root of the trees of 0, 1, 2 and 7 leaves, leaf i = i as u32 LE.
    #[test]
    fn root_vector() {
        assert_eq!(
            hex(&root(&[])),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
        assert_eq!(
            hex(&root(&leaves(1))),
            "cdc96eca844d7912acdbb3dca677757d0db5747a1df61166339cfc7156d4880f"
        );
        assert_eq!(
            hex(&root(&leaves(2))),
            "517102d29c3cffd8ce2d79b86cce32082d658c882432fef056309d9e0f60289f"
        );
        assert_eq!(
            hex(&root(&leaves(7))),
            "3151a19221eafa4693dfec234d4066d45c1f1f011c08aaa8a3109fc7ceca6325"
        );
    }
}
