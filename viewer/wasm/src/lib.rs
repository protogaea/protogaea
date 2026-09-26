//! The core in the browser, for the time machine (roadmap B7): a snapshot of the world is resumed
//! and stepped forward to any later epoch, exactly as the world server stepped it, and the
//! result's `state_root` can be checked against the epoch header the server published. It also
//! checks an organism's inclusion proof from the server against a `state_root` from the log.
//!
//! The interface is a plain C ABI, so the module needs no generated bindings: the page writes the
//! snapshot into memory from `alloc`, calls `load`, `step` and `map`, and reads the results from
//! `out_ptr` and `out_len`.

use std::cell::RefCell;

use protogaea_core::genome::{DEFENSE, HUNTING};
use protogaea_core::merkle::Hash;
use protogaea_core::run::{hex, Run};
use protogaea_core::{Organism, OrganismProof, Ruleset, StateRoots, World};
use serde::Deserialize;
use serde_json::json;

/// What the time machine needs of a server snapshot; the rest (the story detectors) is ignored.
#[derive(Deserialize)]
struct Snapshot {
    seed: u64,
    rules: Ruleset,
    world: World,
}

/// An inclusion proof as `/v0/proofs/{epoch}/organism/{id}` serves it.
#[derive(Deserialize)]
struct ProofJson {
    organism: Organism,
    index: u64,
    size: u64,
    path: Vec<String>,
    roots: RootsJson,
}

#[derive(Deserialize)]
struct RootsJson {
    globals: String,
    cells: String,
    organisms: String,
    clades: String,
    museum: String,
    effects: String,
    rifts: String,
    spore_bank: String,
    revivals: String,
}

#[derive(Deserialize)]
struct VerifyInput {
    proof: ProofJson,
    /// The `state_root` to check against, from the epoch header in the log.
    state_root: String,
}

fn unhex(s: &str) -> Result<Hash, String> {
    if s.len() != 64 || !s.is_ascii() {
        return Err(format!("not a 32-byte hex hash: {s}"));
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// Checks an organism's inclusion proof (spec §15) against a `state_root`.
pub fn verify_proof(input: &[u8]) -> Result<bool, String> {
    let v: VerifyInput = serde_json::from_slice(input).map_err(|e| e.to_string())?;
    let r = &v.proof.roots;
    let proof = OrganismProof {
        organism: v.proof.organism,
        index: v.proof.index,
        size: v.proof.size,
        path: v
            .proof
            .path
            .iter()
            .map(|h| unhex(h))
            .collect::<Result<_, _>>()?,
        roots: StateRoots {
            globals: unhex(&r.globals)?,
            cells: unhex(&r.cells)?,
            organisms: unhex(&r.organisms)?,
            clades: unhex(&r.clades)?,
            museum: unhex(&r.museum)?,
            effects: unhex(&r.effects)?,
            rifts: unhex(&r.rifts)?,
            spore_bank: unhex(&r.spore_bank)?,
            revivals: unhex(&r.revivals)?,
        },
    };
    Ok(proof.verify(&unhex(&v.state_root)?))
}

thread_local! {
    static RUN: RefCell<Option<Run>> = const { RefCell::new(None) };
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn put(bytes: Vec<u8>) {
    OUT.with(|o| *o.borrow_mut() = bytes);
}

/// Resumes a world from a snapshot's JSON. Returns true when it could.
pub fn load_snapshot(json: &[u8]) -> Result<u64, String> {
    let s: Snapshot = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    let run = Run::resume(s.seed, s.rules, s.world);
    let epoch = run.world.epoch;
    RUN.with(|r| *r.borrow_mut() = Some(run));
    Ok(epoch)
}

/// Steps the loaded world `n` epochs. Returns the epoch it is at.
pub fn step_epochs(n: u32) -> u64 {
    RUN.with(|r| {
        let mut r = r.borrow_mut();
        let run = r.as_mut().expect("a world is loaded");
        for _ in 0..n {
            run.step();
        }
        run.world.epoch
    })
}

/// The archetype an organism is counted as: 0 grazer, 1 armored, 2 hunter (as in the server).
fn archetype(traits: &[u8]) -> u8 {
    if traits[HUNTING] >= 4 {
        2
    } else if traits[DEFENSE] >= 6 {
        1
    } else {
        0
    }
}

/// The loaded world in the shape of the server's `/v0/map`, without names, with its `state_root`.
pub fn map_json() -> Vec<u8> {
    RUN.with(|r| {
        let r = r.borrow();
        let run = r.as_ref().expect("a world is loaded");
        let w = &run.world;
        let n = w.organisms.len();
        let mut traits = Vec::with_capacity(n * 6);
        let (mut id, mut cell, mut clade, mut hue, mut kind, mut energy, mut age) = (
            Vec::with_capacity(n),
            Vec::with_capacity(n),
            Vec::with_capacity(n),
            Vec::with_capacity(n),
            Vec::with_capacity(n),
            Vec::with_capacity(n),
            Vec::with_capacity(n),
        );
        for o in &w.organisms {
            id.push(o.id);
            cell.push(o.cell);
            clade.push(o.clade_id);
            hue.push(o.genome.hue);
            kind.push(archetype(&o.genome.traits));
            energy.push(o.energy);
            age.push(o.age);
            traits.extend_from_slice(&o.genome.traits);
        }
        let value = json!({
            "epoch": w.epoch,
            "width": w.width,
            "height": w.height,
            "biome": w.cells.iter().map(|c| c.biome as u8).collect::<Vec<_>>(),
            "food": w.cells.iter().map(|c| c.food).collect::<Vec<_>>(),
            "moisture": w.cells.iter().map(|c| c.moisture).collect::<Vec<_>>(),
            "rift": w.cells.iter().map(|c| c.rift as u8).collect::<Vec<_>>(),
            "effects": w.effects,
            "names": {},
            "organisms": {
                "id": id, "cell": cell, "clade": clade, "hue": hue,
                "kind": kind, "energy": energy, "age": age, "traits": traits,
            },
            "state_root": hex(&run.state_root()),
        });
        serde_json::to_vec(&value).expect("the map serializes")
    })
}

// ---------------------------------------------------------------- the C ABI for the page

/// Memory for the page to write a snapshot into.
#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Frees what `alloc` gave.
///
/// # Safety
/// `ptr` and `len` must come from one call of `alloc`.
#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len));
}

/// Loads a snapshot of `len` bytes at `ptr`. Returns its epoch, or -1 with the error in the output.
///
/// # Safety
/// `ptr` must point to `len` initialized bytes.
#[no_mangle]
pub unsafe extern "C" fn load(ptr: *const u8, len: usize) -> i64 {
    let bytes = std::slice::from_raw_parts(ptr, len);
    match load_snapshot(bytes) {
        Ok(epoch) => epoch as i64,
        Err(e) => {
            put(e.into_bytes());
            -1
        }
    }
}

/// Steps `n` epochs; returns the epoch the world is at.
#[no_mangle]
pub extern "C" fn step(n: u32) -> u64 {
    step_epochs(n)
}

/// Puts the map of the current state in the output.
#[no_mangle]
pub extern "C" fn map() {
    put(map_json());
}

/// Checks the proof JSON of `len` bytes at `ptr` (`{proof, state_root}`): 1 if it holds, 0 if
/// not, -1 with the error in the output.
///
/// # Safety
/// `ptr` must point to `len` initialized bytes.
#[no_mangle]
pub unsafe extern "C" fn verify(ptr: *const u8, len: usize) -> i32 {
    match verify_proof(std::slice::from_raw_parts(ptr, len)) {
        Ok(ok) => i32::from(ok),
        Err(e) => {
            put(e.into_bytes());
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn out_ptr() -> *const u8 {
    OUT.with(|o| o.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn out_len() -> usize {
    OUT.with(|o| o.borrow().len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use protogaea_core::run::Run;

    /// A proof in the server's JSON holds against the right root and fails when anything is changed.
    #[test]
    fn checks_inclusion_proofs() {
        let mut run = Run::new(3, Ruleset::default());
        for _ in 0..5 {
            run.step();
        }
        let o = run.world.organisms[run.world.organisms.len() / 2];
        let p = run.world.prove_organism(o.id).unwrap();
        let roots = |r: &StateRoots| {
            json!({
                "globals": hex(&r.globals), "cells": hex(&r.cells), "organisms": hex(&r.organisms),
                "clades": hex(&r.clades), "museum": hex(&r.museum), "effects": hex(&r.effects),
                "rifts": hex(&r.rifts), "spore_bank": hex(&r.spore_bank), "revivals": hex(&r.revivals),
            })
        };
        let proof = json!({
            "organism": p.organism, "index": p.index, "size": p.size,
            "path": p.path.iter().map(|h| hex(h)).collect::<Vec<_>>(), "roots": roots(&p.roots),
        });
        let root = hex(&run.state_root());
        let check = |proof: &serde_json::Value, root: &str| {
            verify_proof(
                &serde_json::to_vec(&json!({ "proof": proof, "state_root": root })).unwrap(),
            )
        };
        assert_eq!(check(&proof, &root), Ok(true));
        // Another state_root, a changed organism, a changed path: all fail.
        let mut other = root.clone();
        other.replace_range(0..2, if &root[0..2] == "00" { "01" } else { "00" });
        assert_eq!(check(&proof, &other), Ok(false));
        let mut changed = proof.clone();
        changed["organism"]["energy"] = json!(p.organism.energy + 1);
        assert_eq!(check(&changed, &root), Ok(false));
        if !p.path.is_empty() {
            let mut changed = proof.clone();
            changed["path"][0] = json!(hex(&[7u8; 32]));
            assert_eq!(check(&changed, &root), Ok(false));
        }
        assert!(check(&proof, "zz").is_err());
    }

    /// A world resumed from its snapshot and stepped goes exactly where the original went.
    #[test]
    fn resumes_and_steps_like_the_original() {
        let rules = Ruleset::default();
        let mut original = Run::new(7, rules.clone());
        for _ in 0..30 {
            original.step();
        }
        let snapshot = serde_json::to_vec(&json!({
            "seed": 7, "rules": rules, "world": original.world, "detectors": {}
        }))
        .unwrap();
        for _ in 0..12 {
            original.step();
        }
        assert_eq!(load_snapshot(&snapshot).unwrap(), 30);
        assert_eq!(step_epochs(12), 42);
        let map: serde_json::Value = serde_json::from_slice(&map_json()).unwrap();
        assert_eq!(map["epoch"], 42);
        assert_eq!(map["state_root"], hex(&original.state_root()));
        assert_eq!(
            map["organisms"]["id"].as_array().unwrap().len(),
            original.world.organisms.len()
        );
    }
}
