//! The deterministic simulation core of Protogaea.
//!
//! This is consensus code: the same inputs must produce the same bytes on every platform.
//! It follows the rules of spec §14: integers only, counter-based randomness, no iteration
//! over hash tables, explicit overflow handling (overflow checks stay on in release builds)
//! and no system time.
//!
//! Stage A implements the world rules step by step. `core/README.md` lists what is
//! implemented, what is simplified and what is still missing compared with the specification.
#![forbid(unsafe_code)]

pub mod climate;
pub mod genome;
pub mod map;
pub mod merkle;
pub mod rifts;
pub mod rng;
pub mod ruleset;
pub mod run;
pub mod sim;
pub mod state;

pub use genome::Genome;
pub use map::{genesis, world_plan};
pub use ruleset::Ruleset;
pub use sim::{epoch_seed, step_epoch, DeathCause, EpochReport};
pub use state::{
    Biome, Cell, Clade, Effect, EffectKind, MuseumEntry, Organism, OrganismProof, Rift, RiftPhase,
    StateRoots, World,
};
