//! A world run with a stand-in beacon, so that runs are reproducible offline.

use protogaea_core::rifts::Plan;
use protogaea_core::rng::derive;
use protogaea_core::{
    epoch_seed, genesis, step_epoch, world_plan, Biome, EpochReport, Ruleset, World,
};

pub struct Run {
    pub seed: u64,
    pub rules: Ruleset,
    pub world: World,
    /// The terrain at genesis and the rift plan (spec §4), for reports and metrics.
    pub genesis_biomes: Vec<Biome>,
    pub plan: Plan,
    last_hash: [u8; 32],
}

/// The genesis seed and world id of a run seed.
pub fn genesis_inputs(seed: u64) -> ([u8; 32], [u8; 16]) {
    let seed_bytes = seed.to_le_bytes();
    let genesis_seed = derive(b"PROTOGAEA/SIM_GENESIS/V0", &[&seed_bytes]);
    let id = derive(b"PROTOGAEA/SIM_WORLD/V0", &[&seed_bytes]);
    let mut world_id = [0u8; 16];
    world_id.copy_from_slice(&id[..16]);
    (genesis_seed, world_id)
}

impl Run {
    pub fn new(seed: u64, rules: Ruleset) -> Self {
        let (genesis_seed, world_id) = genesis_inputs(seed);
        let world = genesis(&rules, &genesis_seed, world_id);
        Self::resume(seed, rules, world)
    }

    /// Continues a world saved earlier.
    pub fn resume(seed: u64, rules: Ruleset, world: World) -> Self {
        let (genesis_seed, _) = genesis_inputs(seed);
        let (genesis_biomes, plan) = world_plan(&rules, &genesis_seed);
        let last_hash = world.state_hash();
        Self {
            seed,
            rules,
            world,
            genesis_biomes,
            plan,
            last_hash,
        }
    }

    /// The stage A stand-in for the drand beacon (spec §20): derived from the run seed.
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

/// The phase of Season 1 at an epoch (spec §4): its number and name.
pub fn season_phase(rules: &Ruleset, epoch: u64) -> (u8, &'static str) {
    let r = &rules.rifts;
    let day = epoch / u64::from(rules.epochs_per_day);
    let before = |d: u32| day < u64::from(d);
    if before(r.fault_day) {
        (1, "Unity")
    } else if before(r.shallows_from_day) {
        (2, "Cracks")
    } else if before(r.deep_from_day) {
        (3, "Shallows")
    } else if before(r.bridges_from_day) {
        (4, "Straits")
    } else if before(r.bridges_to_day) {
        (5, "The last bridges")
    } else {
        (6, "Continents")
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
