//! The Merkle `state_root` (spec §15): every part of the state changes it, and an organism can be
//! proven against it without the rest of the state.

use protogaea_core::{epoch_seed, genesis, step_epoch, Ruleset, World};

fn world_after(epochs: u64) -> World {
    let rules = Ruleset::default();
    let mut world = genesis(&rules, &[3; 32], [4; 16]);
    for _ in 0..epochs {
        let seed = epoch_seed(&world.world_id, world.epoch, &[9; 32], &world.state_root());
        step_epoch(&mut world, &rules, &seed);
    }
    world
}

#[test]
fn every_part_of_the_state_moves_the_root() {
    let world = world_after(3);
    let root = world.state_root();
    assert_eq!(
        root,
        world.clone().state_root(),
        "the same state, the same root"
    );

    let mut w = world.clone();
    w.organisms[7].energy += 1;
    assert_ne!(w.state_root(), root, "an organism");

    let mut w = world.clone();
    w.cells[100].food += 1;
    assert_ne!(w.state_root(), root, "a cell");

    let mut w = world.clone();
    w.next_clade_id += 1;
    assert_ne!(w.state_root(), root, "a global field");

    let mut w = world.clone();
    w.revivals.push(w.epoch);
    assert_ne!(w.state_root(), root, "a revival");

    // Swapping two organisms' places in the list changes the tree too.
    let mut w = world.clone();
    w.organisms.swap(0, 1);
    assert_ne!(w.state_root(), root, "the order of organisms");
}

#[test]
fn every_organism_proves_against_the_root() {
    let world = world_after(5);
    let root = world.state_root();
    for o in &world.organisms {
        let proof = world.prove_organism(o.id).expect("a living organism");
        assert_eq!(proof.organism, *o);
        assert!(proof.verify(&root), "organism {}", o.id);
    }
    assert!(world.prove_organism(world.next_organism_id).is_none());
}

#[test]
fn a_forged_proof_fails() {
    let world = world_after(5);
    let root = world.state_root();
    let id = world.organisms[world.organisms.len() / 2].id;
    let proof = world.prove_organism(id).unwrap();

    let mut forged = proof.clone();
    forged.organism.energy += 100;
    assert!(!forged.verify(&root), "a changed organism");

    let mut forged = proof.clone();
    forged.roots.cells[0] ^= 1;
    assert!(!forged.verify(&root), "a changed sibling root");

    let later = world_after(6).state_root();
    assert!(!proof.verify(&later), "another epoch's root");
}
