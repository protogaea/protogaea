//! Determinism and invariants over whole epochs.

use protogaea_core::rng::derive;
use protogaea_core::{epoch_seed, genesis, step_epoch, Ruleset, World};

fn start(seed: u64) -> (World, Ruleset) {
    let rules = Ruleset::default();
    let world = genesis(
        &rules,
        &derive(b"test-genesis", &[&seed.to_le_bytes()]),
        [seed as u8; 16],
    );
    (world, rules)
}

/// Runs `epochs` epochs with a stand-in beacon and returns the state hash after each.
fn advance(world: &mut World, rules: &Ruleset, epochs: u64) -> Vec<[u8; 32]> {
    let mut hashes = Vec::new();
    let mut prev = world.state_hash();
    for _ in 0..epochs {
        let beacon = derive(b"test-beacon", &[&world.epoch.to_le_bytes()]);
        let seed = epoch_seed(&world.world_id, world.epoch, &beacon, &prev);
        step_epoch(world, rules, &seed);
        world.check_invariants(rules).unwrap();
        prev = world.state_hash();
        hashes.push(prev);
    }
    hashes
}

#[test]
fn same_seed_same_history() {
    let (mut a, rules) = start(1);
    let (mut b, _) = start(1);
    assert_eq!(advance(&mut a, &rules, 12), advance(&mut b, &rules, 12));
    assert_eq!(a, b);
}

#[test]
fn different_seeds_differ() {
    let (mut a, rules) = start(1);
    let (mut b, _) = start(2);
    assert_ne!(advance(&mut a, &rules, 3), advance(&mut b, &rules, 3));
}

#[test]
fn continuing_from_a_copy_gives_the_same_result() {
    let (mut world, rules) = start(3);
    advance(&mut world, &rules, 6);
    let mut copy = world.clone();
    assert_eq!(
        advance(&mut world, &rules, 6),
        advance(&mut copy, &rules, 6)
    );
}

#[test]
fn the_world_lives_and_changes() {
    let (mut world, rules) = start(4);
    let founders = world.organisms.len();
    advance(&mut world, &rules, 24);
    assert!(
        !world.organisms.is_empty(),
        "everything died within two hours"
    );
    assert!(
        world.next_organism_id as usize > founders + 1,
        "nobody was born"
    );
}
