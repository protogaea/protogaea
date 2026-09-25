//! A world run with a stand-in beacon, so that runs are reproducible offline.

use protogaea_core::rng::derive;
use protogaea_core::{epoch_seed, genesis, step_epoch, EpochReport, Ruleset, World};

pub struct Run {
    pub seed: u64,
    pub rules: Ruleset,
    pub world: World,
    last_hash: [u8; 32],
}

impl Run {
    pub fn new(seed: u64, rules: Ruleset) -> Self {
        let seed_bytes = seed.to_le_bytes();
        let genesis_seed = derive(b"PROTOGAEA/SIM_GENESIS/V0", &[&seed_bytes]);
        let id = derive(b"PROTOGAEA/SIM_WORLD/V0", &[&seed_bytes]);
        let mut world_id = [0u8; 16];
        world_id.copy_from_slice(&id[..16]);
        let world = genesis(&rules, &genesis_seed, world_id);
        let last_hash = world.state_hash();
        Self {
            seed,
            rules,
            world,
            last_hash,
        }
    }

    /// Continues a world saved earlier.
    pub fn resume(seed: u64, rules: Ruleset, world: World) -> Self {
        let last_hash = world.state_hash();
        Self {
            seed,
            rules,
            world,
            last_hash,
        }
    }

    /// The stage A1 stand-in for the drand beacon (spec §20): derived from the run seed.
    fn beacon(&self, epoch: u64) -> [u8; 32] {
        derive(
            b"PROTOGAEA/SIM_BEACON/V0",
            &[&self.seed.to_le_bytes(), &epoch.to_le_bytes()],
        )
    }

    pub fn step(&mut self) -> EpochReport {
        let epoch = self.world.epoch;
        let seed = epoch_seed(
            &self.world.world_id,
            epoch,
            &self.beacon(epoch),
            &self.last_hash,
        );
        let report = step_epoch(&mut self.world, &self.rules, &seed);
        self.last_hash = self.world.state_hash();
        report
    }

    /// The hash of the current state; it also serves as the previous header hash.
    pub fn state_hash(&self) -> [u8; 32] {
        self.last_hash
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
