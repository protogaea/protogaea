//! The proof of work behind sparks (spec §16, §18).
//!
//! A spark is a nonce that makes yespower 1.0 of the spark input fall below the epoch target. The
//! hash is the reference C implementation by Alexander Peslyak (Openwall), vendored unchanged in
//! `yespower/` under its 2-clause BSD license; this crate adds the Protogaea parameters, the
//! spark input of §18 and the target check.

use std::ffi::c_void;

/// yespower 1.0, as opposed to the older yescrypt 0.5 variant.
const YESPOWER_1_0: u32 = 10;

/// The spark parameters of spec §16: N = 2048, r = 32 (8 MiB of memory per thread) and a
/// personalization that separates Protogaea's work from any other use of yespower.
pub const SPARK: Params = Params {
    n: 2048,
    r: 32,
    pers: b"PROTOGAEA/SPARK/V0",
};

/// The domain tag at the start of every spark input.
pub const SPARK_TAG: &[u8] = b"PROTOGAEA/SPARK/V0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    pub n: u32,
    pub r: u32,
    pub pers: &'static [u8],
}

impl Params {
    /// The memory one hash works in: 128 · N · r bytes.
    pub fn memory(&self) -> usize {
        128 * self.n as usize * self.r as usize
    }
}

#[repr(C)]
struct Region {
    base: *mut c_void,
    aligned: *mut c_void,
    base_size: usize,
    aligned_size: usize,
}

#[repr(C)]
struct RawParams {
    version: u32,
    n: u32,
    r: u32,
    pers: *const u8,
    perslen: usize,
}

extern "C" {
    fn yespower_init_local(local: *mut Region) -> i32;
    fn yespower_free_local(local: *mut Region) -> i32;
    fn yespower(
        local: *mut Region,
        src: *const u8,
        srclen: usize,
        params: *const RawParams,
        dst: *mut [u8; 32],
    ) -> i32;
}

/// A yespower hasher with its working memory, kept between hashes. One per thread.
pub struct Hasher {
    local: Region,
    params: Params,
}

// The working memory belongs to the hasher alone; it may move between threads but not be shared.
unsafe impl Send for Hasher {}

impl Hasher {
    pub fn new(params: Params) -> Self {
        let mut local = Region {
            base: std::ptr::null_mut(),
            aligned: std::ptr::null_mut(),
            base_size: 0,
            aligned_size: 0,
        };
        // SAFETY: `local` is a valid region for the library to initialize.
        let rc = unsafe { yespower_init_local(&mut local) };
        assert_eq!(rc, 0, "yespower_init_local failed");
        Self { local, params }
    }

    /// yespower 1.0 of `input` with this hasher's parameters.
    pub fn hash(&mut self, input: &[u8]) -> [u8; 32] {
        let raw = RawParams {
            version: YESPOWER_1_0,
            n: self.params.n,
            r: self.params.r,
            pers: self.params.pers.as_ptr(),
            perslen: self.params.pers.len(),
        };
        let mut out = [0u8; 32];
        // SAFETY: the pointers are valid for the lengths given; `local` was initialized in `new`.
        let rc = unsafe { yespower(&mut self.local, input.as_ptr(), input.len(), &raw, &mut out) };
        assert_eq!(rc, 0, "yespower failed (out of memory?)");
        out
    }
}

impl Drop for Hasher {
    fn drop(&mut self) {
        // SAFETY: `local` was initialized in `new` and is freed once.
        unsafe { yespower_free_local(&mut self.local) };
    }
}

/// The PoW input of a spark (spec §18), 146 bytes:
/// tag ‖ world_id ‖ epoch (u64 LE) ‖ challenge ‖ proposal_id ‖ miner_pubkey ‖ nonce (u64 LE).
pub fn spark_input(
    world_id: &[u8; 16],
    epoch: u64,
    challenge: &[u8; 32],
    proposal_id: &[u8; 32],
    miner_pubkey: &[u8; 32],
    nonce: u64,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(SPARK_TAG.len() + 16 + 8 + 32 * 3 + 8);
    out.extend_from_slice(SPARK_TAG);
    out.extend_from_slice(world_id);
    out.extend_from_slice(&epoch.to_le_bytes());
    out.extend_from_slice(challenge);
    out.extend_from_slice(proposal_id);
    out.extend_from_slice(miner_pubkey);
    out.extend_from_slice(&nonce.to_le_bytes());
    out
}

/// Whether a hash is a spark under the target: its first 8 bytes, big-endian, below `target`.
pub fn meets(hash: &[u8; 32], target: u64) -> bool {
    u64::from_be_bytes(hash[..8].try_into().expect("8 bytes")) < target
}

/// A spark's weight in work units: the expected number of hashes to find one, ⌊2⁶⁴ / target⌋.
pub fn weight(target: u64) -> u64 {
    assert!(target > 0, "a zero target admits no spark");
    ((1u128 << 64) / u128::from(target)).min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// The input of the reference tests: 80 bytes, byte i = 3i.
    fn reference_input() -> Vec<u8> {
        (0..80u32).map(|i| (i * 3) as u8).collect()
    }

    /// The yespower 1.0 vectors of the reference `TESTS-OK`.
    #[test]
    fn matches_the_reference_vectors() {
        let cases: [(u32, u32, &'static [u8], &str); 6] = [
            (
                2048,
                8,
                b"",
                "69e0e895b3df7aeeb837d71fe199e9d34f7ec46ecbca7a2c4308e51857ae9b46",
            ),
            (
                4096,
                16,
                b"",
                "33fb8f063824a4a020f63dca535f5ca66ab5576468c75d1ccaac7542f76495ac",
            ),
            (
                4096,
                32,
                b"",
                "771aeefda8fe79a0825bc7f2aee162ab5578574639ffc6ca3723cc18e5e3e285",
            ),
            (
                2048,
                32,
                b"",
                "d5efb813cd263e9b34540130233cbbc6a921fbff3431e5ec1a1abde2aea6ff4d",
            ),
            (
                1024,
                32,
                b"",
                "501b792db42e388f6e7d453c95d03a12a36016a5154a688390ddc609a40c6799",
            ),
            (
                1024,
                32,
                b"personality test",
                "1f0269acf565c49adc0ef9b8f26ab3808cdc38394a254fddeedcc3aacff6ad9d",
            ),
        ];
        let input = reference_input();
        for (n, r, pers, want) in cases {
            let mut h = Hasher::new(Params { n, r, pers });
            assert_eq!(hex(&h.hash(&input)), want, "N = {n}, r = {r}");
        }
    }

    /// The Protogaea parameters: the personalization changes the hash, and the input layout is
    /// the one of §18.
    #[test]
    fn spark_hash_and_input() {
        let input = spark_input(&[1; 16], 7, &[2; 32], &[3; 32], &[4; 32], 0x0102);
        assert_eq!(input.len(), 146);
        assert_eq!(&input[..18], SPARK_TAG);
        assert_eq!(&input[34..42], &7u64.to_le_bytes());
        assert_eq!(&input[138..], &0x0102u64.to_le_bytes());
        let mut spark = Hasher::new(SPARK);
        let mut plain = Hasher::new(Params { pers: b"", ..SPARK });
        let a = spark.hash(&input);
        assert_eq!(a, spark.hash(&input), "deterministic");
        assert_ne!(
            a,
            plain.hash(&input),
            "the personalization separates the domains"
        );
        assert_eq!(SPARK.memory(), 8 << 20);
    }

    #[test]
    fn targets_and_weights() {
        let mut hash = [0u8; 32];
        hash[..8].copy_from_slice(&1000u64.to_be_bytes());
        assert!(meets(&hash, 1001));
        assert!(!meets(&hash, 1000));
        assert_eq!(weight(1 << 60), 16);
        assert_eq!(weight(u64::MAX), 1);
        assert_eq!(weight(1), u64::MAX);
    }
}
