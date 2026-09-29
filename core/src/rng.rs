//! Counter-based randomness (spec §14, decision 0014).
//!
//! The generator has no state. Every number is computed directly by Philox4x32-10 (Salmon et
//! al., "Parallel random numbers: as easy as 1, 2, 3", SC11; checked against Random123's
//! known-answer vectors):
//!
//! - the key, once per seed: the first 8 bytes of `BLAKE3("PROTOGAEA/RAND/V1" ‖ seed)`, as two
//!   little-endian `u32` words;
//! - the counter: `[tick, subject_lo, subject_hi, purpose | k << 8]`, with `subject: u64` split
//!   into its low and high halves, `purpose < 256` and `k < 2^24`;
//! - the value: output words 0 and 1 as `w0 | w1 << 32`.
//!
//! Results therefore do not depend on the order of calls. It replaced one BLAKE3 hash per draw
//! (`PROTOGAEA/RAND/V0`), which took about 60% of a tick; Philox is about 11 times cheaper,
//! natively and in WebAssembly, and passes BigCrush with rounds to spare.

const RAND_TAG: &[u8] = b"PROTOGAEA/RAND/V1";
/// `k` shares the last counter word with the purpose.
const K_LIMIT: u32 = 1 << 24;

const PHILOX_M0: u32 = 0xD251_1F53;
const PHILOX_M1: u32 = 0xCD9E_8D57;
const PHILOX_W0: u32 = 0x9E37_79B9;
const PHILOX_W1: u32 = 0xBB67_AE85;

/// Philox4x32 with 10 rounds.
fn philox4x32_10(ctr: [u32; 4], key: [u32; 2]) -> [u32; 4] {
    fn mulhilo(a: u32, b: u32) -> (u32, u32) {
        let p = u64::from(a) * u64::from(b);
        ((p >> 32) as u32, p as u32)
    }
    let (mut x, mut k) = (ctr, key);
    for round in 0..10 {
        if round > 0 {
            k[0] = k[0].wrapping_add(PHILOX_W0);
            k[1] = k[1].wrapping_add(PHILOX_W1);
        }
        let (hi0, lo0) = mulhilo(PHILOX_M0, x[0]);
        let (hi1, lo1) = mulhilo(PHILOX_M1, x[2]);
        x = [hi1 ^ x[1] ^ k[0], lo1, hi0 ^ x[3] ^ k[1], lo0];
    }
    x
}

/// What a random number is used for. Each call site has its own purpose, so draws never
/// collide. The values are part of consensus: never renumber them within a ruleset version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Purpose {
    /// The order in which organisms act within a tick; the subject is the organism.
    Queue = 1,
    /// A tie between equally scored target cells; `k` is the candidate cell's index.
    MoveTie = 2,
    /// The attacker's roll; the subject is the attacker.
    AttackRoll = 3,
    /// The defender's roll; the subject is the attacker.
    DefenseRoll = 4,
    /// Death from old age; the subject is the organism.
    Senescence = 5,
    /// The neighboring cell for a newborn; the subject is the child.
    Placement = 6,
    /// Whether a trait step happens; the subject is the child.
    MutationChance = 7,
    /// Which valid pair of traits steps.
    MutationPair = 8,
    /// Whether a behavioral gene changes.
    BehaviorChance = 9,
    /// Which behavioral gene changes.
    BehaviorGene = 10,
    /// The behavioral gene's new value or direction.
    BehaviorValue = 11,
    /// Whether the hue drifts.
    HueChance = 12,
    /// The size and direction of the hue drift.
    HueStep = 13,
    /// Starting ages at genesis; the subject is the organism.
    GenesisAge = 14,
    /// The starting site of a founder lineage; the subject is the lineage.
    GenesisSite = 15,
    /// Terrain height noise; the subject encodes the lattice point.
    MapHeight = 16,
    /// Terrain moisture noise; the subject encodes the lattice point.
    MapMoisture = 17,
    /// Whether a wildfire starts this epoch; the subject is the epoch.
    WildfireChance = 18,
    /// Where it starts.
    WildfireSite = 19,
    /// How far it spreads.
    WildfireRadius = 20,
    /// Whether a great drought starts this epoch; the subject is the epoch.
    DroughtChance = 21,
    /// Where it starts.
    DroughtSite = 22,
    /// Whether a plague strikes this epoch; the subject is the epoch.
    PlagueChance = 23,
    /// Which member of the clade is at its center.
    PlagueSite = 24,
    /// Whether a member within reach dies; the subject is the organism.
    PlagueDeath = 25,
    /// Whether a flood starts this epoch; the subject is the epoch.
    FloodChance = 26,
    /// Where it starts.
    FloodSite = 27,
    /// The number of plates at genesis.
    RiftPlates = 28,
    /// The rotation of the plate layout at genesis.
    RiftRotation = 29,
    /// Noise that bends plate boundaries sideways; the subject encodes the lattice point.
    RiftWarpX = 30,
    /// Noise that bends plate boundaries up and down.
    RiftWarpY = 31,
    /// The center of a land bridge; the subject is the pair of plates it joins.
    BridgeSite = 32,
    /// The order in which land bridges close.
    BridgeOrder = 33,
    /// The cell for an organism revived from the spore bank; the subject is the organism.
    RevivalSite = 34,
    /// Which biome mix goes to which plate at genesis.
    PlateMix = 35,
    /// Whether a newborn loses a gift; `k` is the gift's bit.
    GiftLoss = 36,
    /// Whether a member of a clade under `sickness` dies this tick.
    Sickness = 37,
    /// Natural weather: whether a region gets it this epoch (the subject is the region), where,
    /// and whether it is rain.
    WeatherChance = 38,
    WeatherSite = 39,
    WeatherKind = 40,
}

/// A stateless source of randomness bound to one seed.
#[derive(Clone)]
pub struct Rng {
    key: [u32; 2],
}

impl Rng {
    pub fn new(seed: &[u8; 32]) -> Self {
        let digest = derive(RAND_TAG, &[seed]);
        let word = |i: usize| u32::from_le_bytes(digest[i..i + 4].try_into().expect("4 bytes"));
        Self {
            key: [word(0), word(4)],
        }
    }

    /// The raw 64-bit value for one `(tick, purpose, subject, k)`.
    pub fn raw(&self, tick: u32, purpose: Purpose, subject: u64, k: u32) -> u64 {
        assert!(k < K_LIMIT, "k must stay below 2^24");
        let ctr = [
            tick,
            subject as u32,
            (subject >> 32) as u32,
            purpose as u32 | k << 8,
        ];
        let x = philox4x32_10(ctr, self.key);
        u64::from(x[0]) | u64::from(x[1]) << 32
    }

    /// A uniform number in `[0, n)`. Values that would bias the result are rejected, and the
    /// next attempt uses `k + 1`.
    pub fn below(&self, tick: u32, purpose: Purpose, subject: u64, n: u64) -> u64 {
        assert!(n > 0, "below: empty range");
        let reject_under = n.wrapping_neg() % n; // 2^64 mod n
        let mut k = 0u32;
        loop {
            let x = self.raw(tick, purpose, subject, k);
            if x >= reject_under {
                return x % n;
            }
            k = k.checked_add(1).expect("rejection sampling exhausted");
        }
    }

    /// True with probability `ppm / 1_000_000`.
    pub fn chance_ppm(&self, tick: u32, purpose: Purpose, subject: u64, ppm: u32) -> bool {
        match ppm {
            0 => false,
            p if p >= 1_000_000 => true,
            p => self.below(tick, purpose, subject, 1_000_000) < u64::from(p),
        }
    }
}

/// `BLAKE3(tag ‖ parts…)`, used to derive seeds and identifiers.
pub fn derive(tag: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(tag);
    for part in parts {
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng() -> Rng {
        Rng::new(&derive(b"test", &[]))
    }

    #[test]
    fn same_inputs_same_value() {
        let r = rng();
        assert_eq!(
            r.raw(3, Purpose::Queue, 42, 0),
            r.raw(3, Purpose::Queue, 42, 0)
        );
    }

    #[test]
    fn every_input_matters() {
        let r = rng();
        let base = r.raw(3, Purpose::Queue, 42, 0);
        assert_ne!(base, r.raw(4, Purpose::Queue, 42, 0));
        assert_ne!(base, r.raw(3, Purpose::MoveTie, 42, 0));
        assert_ne!(base, r.raw(3, Purpose::Queue, 43, 0));
        assert_ne!(base, r.raw(3, Purpose::Queue, 42, 1));
        let other = Rng::new(&derive(b"other", &[]));
        assert_ne!(base, other.raw(3, Purpose::Queue, 42, 0));
    }

    #[test]
    fn below_stays_in_range() {
        let r = rng();
        for n in [1u64, 2, 3, 7, 1_000_000, u64::MAX] {
            for subject in 0..200 {
                assert!(r.below(0, Purpose::Placement, subject, n) < n);
            }
        }
    }

    #[test]
    fn below_is_roughly_uniform() {
        let r = rng();
        let mut buckets = [0u32; 6];
        for subject in 0..6000 {
            buckets[r.below(0, Purpose::Placement, subject, 6) as usize] += 1;
        }
        for count in buckets {
            assert!((850..=1150).contains(&count), "bucket count {count}");
        }
    }

    /// Random123's known-answer vectors for Philox4x32-10 (tests/kat_vectors).
    #[test]
    fn philox_known_answers() {
        assert_eq!(
            philox4x32_10([0; 4], [0; 2]),
            [0x6627_e8d5, 0xe169_c58d, 0xbc57_ac4c, 0x9b00_dbd8]
        );
        assert_eq!(
            philox4x32_10([u32::MAX; 4], [u32::MAX; 2]),
            [0x408f_276d, 0x41c8_3b0e, 0xa20b_c7c6, 0x6d54_51fd]
        );
        assert_eq!(
            philox4x32_10(
                [0x243f_6a88, 0x85a3_08d3, 0x1319_8a2e, 0x0370_7344],
                [0xa409_3822, 0x299f_31d0]
            ),
            [0xd16c_fe09, 0x94fd_cceb, 0x5001_e420, 0x2412_6ea1]
        );
    }

    /// The draw of the spec, fixed: a change here changes every world.
    #[test]
    fn draw_vector() {
        let r = Rng::new(&[0; 32]);
        assert_eq!(
            r.raw(0, Purpose::Queue, 0, 0),
            r.raw(0, Purpose::Queue, 0, 0)
        );
        assert_eq!(
            format!("{:016x}", r.raw(7, Purpose::MoveTie, 0x1_0000_0002, 3)),
            DRAW_VECTOR
        );
    }

    const DRAW_VECTOR: &str = "415e167b1d774b8c";

    #[test]
    fn purposes_fit_a_byte() {
        assert!((Purpose::WeatherKind as u32) < 256);
    }

    #[test]
    fn chance_edges() {
        let r = rng();
        assert!(!r.chance_ppm(0, Purpose::Senescence, 1, 0));
        assert!(r.chance_ppm(0, Purpose::Senescence, 1, 1_000_000));
    }
}
