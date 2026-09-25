//! Merkle trees over the state (spec §15), shaped as in RFC 9162 (Certificate Transparency):
//! a leaf is `H(0x00 ‖ domain ‖ data)`, a node `H(0x01 ‖ left ‖ right)`, and a tree of `n`
//! leaves splits at the largest power of two below `n`, so no leaf is ever duplicated. An inclusion
//! proof is the audit path of RFC 9162 §2.1.3.

const LEAF: u8 = 0x00;
const NODE: u8 = 0x01;

pub type Hash = [u8; 32];

/// The hash of one leaf: its kind's domain prefix and its canonical bytes.
pub fn leaf_hash(domain: &[u8], data: &[u8]) -> Hash {
    let mut h = blake3::Hasher::new();
    h.update(&[LEAF]);
    h.update(domain);
    h.update(data);
    *h.finalize().as_bytes()
}

fn node_hash(left: &Hash, right: &Hash) -> Hash {
    let mut h = blake3::Hasher::new();
    h.update(&[NODE]);
    h.update(left);
    h.update(right);
    *h.finalize().as_bytes()
}

/// The root of a tree with no leaves, distinct for each kind of leaf.
pub fn empty_root(domain: &[u8]) -> Hash {
    let mut h = blake3::Hasher::new();
    h.update(b"PROTOGAEA/MERKLE/EMPTY/");
    h.update(domain);
    *h.finalize().as_bytes()
}

/// The largest power of two strictly below `n` (n ≥ 2).
fn split(n: usize) -> usize {
    let mut k = 1;
    while k * 2 < n {
        k *= 2;
    }
    k
}

/// The root over leaf hashes, in order.
pub fn root(leaves: &[Hash], domain: &[u8]) -> Hash {
    match leaves.len() {
        0 => empty_root(domain),
        _ => subtree_root(leaves),
    }
}

fn subtree_root(leaves: &[Hash]) -> Hash {
    if leaves.len() == 1 {
        return leaves[0];
    }
    let k = split(leaves.len());
    node_hash(&subtree_root(&leaves[..k]), &subtree_root(&leaves[k..]))
}

/// The audit path of leaf `index`: the sibling hashes from the leaf up to the root.
pub fn proof(leaves: &[Hash], index: usize) -> Vec<Hash> {
    let mut path = Vec::new();
    audit_path(leaves, index, &mut path);
    path
}

fn audit_path(leaves: &[Hash], index: usize, path: &mut Vec<Hash>) {
    if leaves.len() <= 1 {
        return;
    }
    let k = split(leaves.len());
    if index < k {
        audit_path(&leaves[..k], index, path);
        path.push(subtree_root(&leaves[k..]));
    } else {
        audit_path(&leaves[k..], index - k, path);
        path.push(subtree_root(&leaves[..k]));
    }
}

/// Checks that `leaf` is leaf `index` of a tree of `size` leaves with this root (RFC 9162
/// §2.1.3.2).
pub fn verify(leaf: &Hash, index: u64, size: u64, path: &[Hash], root: &Hash) -> bool {
    if index >= size {
        return false;
    }
    let (mut f, mut s) = (index, size - 1);
    let mut r = *leaf;
    for p in path {
        if s == 0 {
            return false;
        }
        if f & 1 == 1 || f == s {
            r = node_hash(p, &r);
            while f & 1 == 0 && f != 0 {
                f >>= 1;
                s >>= 1;
            }
        } else {
            r = node_hash(&r, p);
        }
        f >>= 1;
        s >>= 1;
    }
    s == 0 && r == *root
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaves(n: usize) -> Vec<Hash> {
        (0..n as u32)
            .map(|i| leaf_hash(b"T", &i.to_le_bytes()))
            .collect()
    }

    #[test]
    fn small_trees_have_the_rfc_shape() {
        let l = leaves(3);
        assert_eq!(root(&l[..1], b"T"), l[0]);
        assert_eq!(root(&l[..2], b"T"), node_hash(&l[0], &l[1]));
        // Three leaves split 2 + 1: the third leaf is promoted, not duplicated.
        assert_eq!(root(&l, b"T"), node_hash(&node_hash(&l[0], &l[1]), &l[2]));
        assert_ne!(root(&[], b"A"), root(&[], b"B"));
    }

    #[test]
    fn every_leaf_proves_and_nothing_else_does() {
        for n in 1..40 {
            let l = leaves(n);
            let r = root(&l, b"T");
            for i in 0..n {
                let path = proof(&l, i);
                assert!(
                    verify(&l[i], i as u64, n as u64, &path, &r),
                    "n {n}, leaf {i}"
                );
                // The wrong leaf or the wrong position fails. (A wrong size can fold to the
                // same root, as in RFC 9162: the size is committed separately, in the state's
                // global leaf.)
                let other = leaf_hash(b"T", b"forged");
                assert!(!verify(&other, i as u64, n as u64, &path, &r));
                if n > 1 {
                    assert!(!verify(&l[i], ((i + 1) % n) as u64, n as u64, &path, &r));
                }
            }
        }
    }
}
