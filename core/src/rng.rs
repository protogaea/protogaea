//! Counter-based randomness (spec §14).
//!
//! The generator has no state. Every number is computed directly as
//! `u64_le(BLAKE3("PROTOGAEA/RAND/V0" ‖ seed ‖ tick ‖ subject ‖ purpose ‖ k)[0..8])`,
//! where `tick: u32`, `subject: u64`, `purpose: u32` and `k: u32` are little-endian.
//! Results therefore do not depend on the order of calls.

const RAND_TAG: &[u8] = b"PROTOGAEA/RAND/V0";
const PREFIX_LEN: usize = RAND_TAG.len() + 32;
const INPUT_LEN: usize = PREFIX_LEN + 4 + 8 + 4 + 4;

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
}

/// A stateless source of randomness bound to one seed.
#[derive(Clone)]
pub struct Rng {
    prefix: [u8; PREFIX_LEN],
}

impl Rng {
    pub fn new(seed: &[u8; 32]) -> Self {
        let mut prefix = [0u8; PREFIX_LEN];
        prefix[..RAND_TAG.len()].copy_from_slice(RAND_TAG);
        prefix[RAND_TAG.len()..].copy_from_slice(seed);
        Self { prefix }
    }

    /// The raw 64-bit value for one `(tick, purpose, subject, k)`.
    pub fn raw(&self, tick: u32, purpose: Purpose, subject: u64, k: u32) -> u64 {
        let mut input = [0u8; INPUT_LEN];
        input[..PREFIX_LEN].copy_from_slice(&self.prefix);
        input[PREFIX_LEN..PREFIX_LEN + 4].copy_from_slice(&tick.to_le_bytes());
        input[PREFIX_LEN + 4..PREFIX_LEN + 12].copy_from_slice(&subject.to_le_bytes());
        input[PREFIX_LEN + 12..PREFIX_LEN + 16].copy_from_slice(&(purpose as u32).to_le_bytes());
        input[PREFIX_LEN + 16..].copy_from_slice(&k.to_le_bytes());
        let hash = blake3::hash(&input);
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&hash.as_bytes()[..8]);
        u64::from_le_bytes(bytes)
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

    #[test]
    fn chance_edges() {
        let r = rng();
        assert!(!r.chance_ppm(0, Purpose::Senescence, 1, 0));
        assert!(r.chance_ppm(0, Purpose::Senescence, 1, 1_000_000));
    }
}
