//! The proof of work behind sparks (spec §16, §18).
//!
//! A spark is a nonce that makes yespower 1.0 of the spark input fall below the epoch target. The
//! hash is yespower 1.0 by Alexander Peslyak (Openwall), ported to plain Rust from the reference
//! implementation (`portable`), so that it builds anywhere, WebAssembly included. The reference C
//! implementation is vendored unchanged in `yespower/` under its 2-clause BSD license and built
//! with the `c` feature, to check the port against it. This crate adds the Protogaea parameters,
//! the spark input of §18 and the target check.

#[cfg(feature = "c")]
pub mod c;
mod portable;

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

/// A yespower 1.0 hasher with its working memory, kept between hashes. One per thread.
pub struct Hasher(portable::Hasher);

impl Hasher {
    pub fn new(params: Params) -> Self {
        Self(portable::Hasher::new(params.n, params.r, params.pers))
    }

    /// yespower 1.0 of `input` with this hasher's parameters.
    pub fn hash(&mut self, input: &[u8]) -> [u8; 32] {
        self.0.hash(input)
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

    /// The port and the reference C implementation agree on the spark parameters and many inputs.
    #[cfg(feature = "c")]
    #[test]
    fn agrees_with_the_reference_c() {
        let mut rust = Hasher::new(SPARK);
        let mut c = crate::c::Hasher::new(SPARK);
        for nonce in 0..40u64 {
            let input = spark_input(
                &[9; 16],
                nonce * 7,
                &[nonce as u8; 32],
                &[5; 32],
                &[6; 32],
                nonce,
            );
            assert_eq!(rust.hash(&input), c.hash(&input), "nonce {nonce}");
        }
        let mut rust = Hasher::new(Params {
            n: 1024,
            r: 8,
            pers: b"",
        });
        let mut c = crate::c::Hasher::new(Params {
            n: 1024,
            r: 8,
            pers: b"",
        });
        for len in [0usize, 1, 63, 64, 65, 200] {
            let input: Vec<u8> = (0..len).map(|i| (i * 31 + 7) as u8).collect();
            assert_eq!(rust.hash(&input), c.hash(&input), "length {len}");
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

    /// A Protogaea spark vector: yespower with the spark parameters of 146 bytes, byte i = 7i + 3.
    /// The browser build (WebAssembly) gives the same hash.
    #[test]
    fn spark_vector() {
        let input: Vec<u8> = (0..146u32).map(|i| (i * 7 + 3) as u8).collect();
        assert_eq!(
            hex(&Hasher::new(SPARK).hash(&input)),
            "8f4d79040f183a82372d19fe721a71b478ea26b5a6f2b1322bba83040cc9c727"
        );
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
