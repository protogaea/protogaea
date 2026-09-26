//! The core in the browser, for the time machine (roadmap B7): a snapshot of the world is resumed
//! and stepped forward to any later epoch, exactly as the world server stepped it, and the
//! result's `state_root` can be checked against the epoch header the server published.
//!
//! The interface is a plain C ABI, so the module needs no generated bindings: the page writes the
//! snapshot into memory from `alloc`, calls `load`, `step` and `map`, and reads the results from
//! `out_ptr` and `out_len`.

use std::cell::RefCell;

use protogaea_core::genome::{DEFENSE, HUNTING};
use protogaea_core::run::{hex, Run};
use protogaea_core::{Ruleset, World};
use serde::Deserialize;
use serde_json::json;

/// What the time machine needs of a server snapshot; the rest (the story detectors) is ignored.
#[derive(Deserialize)]
struct Snapshot {
    seed: u64,
    rules: Ruleset,
    world: World,
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
