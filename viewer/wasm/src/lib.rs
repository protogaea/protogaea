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
    /// Present only while miracles' cooldowns run.
    #[serde(default)]
    cooldowns: Option<String>,
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
            cooldowns: r.cooldowns.as_deref().map(unhex).transpose()?,
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

/// Steps the loaded world one epoch with the miracles given for it (a JSON array of the core's
/// `Miracle`, as `/v0/miracles` serves them, in their order). Returns the epoch it is at.
pub fn step_with_miracles(json: &[u8]) -> Result<u64, String> {
    let miracles: Vec<protogaea_core::Miracle> =
        serde_json::from_slice(json).map_err(|e| e.to_string())?;
    Ok(RUN.with(|r| {
        let mut r = r.borrow_mut();
        let run = r.as_mut().expect("a world is loaded");
        run.step_with(&miracles);
        run.world.epoch
    }))
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

/// Steps one epoch with the miracles in the JSON of `len` bytes at `ptr`. Returns the epoch, or
/// -1 with the error in the output.
///
/// # Safety
/// `ptr` must point to `len` initialized bytes.
#[no_mangle]
pub unsafe extern "C" fn step_with(ptr: *const u8, len: usize) -> i64 {
    match step_with_miracles(std::slice::from_raw_parts(ptr, len)) {
        Ok(epoch) => epoch as i64,
        Err(e) => {
            put(e.into_bytes());
            -1
        }
    }
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

thread_local! {
    static SPARK_HASHER: RefCell<Option<protogaea_pow::Hasher>> = const { RefCell::new(None) };
}

/// yespower of `len` bytes at `ptr` with the spark parameters (spec §16); the 32-byte hash goes
/// to the output. The hasher's 8 MiB are kept between calls.
///
/// # Safety
/// `ptr` must point to `len` initialized bytes.
#[no_mangle]
pub unsafe extern "C" fn spark_hash(ptr: *const u8, len: usize) {
    let input = std::slice::from_raw_parts(ptr, len);
    let hash = SPARK_HASHER.with(|h| {
        h.borrow_mut()
            .get_or_insert_with(|| protogaea_pow::Hasher::new(protogaea_pow::SPARK))
            .hash(input)
    });
    put(hash.to_vec());
}

/// Hashes `count` spark inputs with successive nonces, for measuring the rate in a browser.
#[no_mangle]
pub extern "C" fn spark_bench(count: u32) {
    SPARK_HASHER.with(|h| {
        let mut h = h.borrow_mut();
        let hasher = h.get_or_insert_with(|| protogaea_pow::Hasher::new(protogaea_pow::SPARK));
        for nonce in 0..u64::from(count) {
            let input =
                protogaea_pow::spark_input(&[7; 16], 1, &[1; 32], &[2; 32], &[3; 32], nonce);
            std::hint::black_box(hasher.hash(&input));
        }
    });
}

// ---------------------------------------------------------------- the spark client (stage C)

fn hex_arr<const N: usize>(v: &serde_json::Value) -> Result<[u8; N], String> {
    unhex_n(v.as_str().ok_or("expected a hex string")?)
}

fn unhex_n<const N: usize>(s: &str) -> Result<[u8; N], String> {
    if s.len() != 2 * N || !s.is_ascii() {
        return Err(format!("expected {N} bytes of hex"));
    }
    let mut out = [0u8; N];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

fn num(v: &serde_json::Value) -> Result<u64, String> {
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .ok_or_else(|| "expected a number".to_string())
}

/// The wish of `req.action`: `{"weather": {x, y, rain}}`, `{"migrate": {clade_id, from, to}}` or
/// `{"revive": {museum, entry_id, steps, at}}`, with points as `[x, y]`.
fn wish_of(req: &serde_json::Value) -> Result<protogaea_protocol::wish::Wish, String> {
    use protogaea_protocol::wish::{Action, Source, Weather, Wish};
    let point = |v: &serde_json::Value| -> Result<(u8, u8), String> {
        let a = v.as_array().ok_or("a point is [x, y]")?;
        Ok((num(&a[0])? as u8, num(&a[1])? as u8))
    };
    let a = &req["action"];
    let action = if let Some(w) = a.get("weather") {
        Action::Weather {
            x: num(&w["x"])? as u8,
            y: num(&w["y"])? as u8,
            kind: if w["rain"].as_bool().unwrap_or(true) {
                Weather::Rain
            } else {
                Weather::Drought
            },
        }
    } else if let Some(m) = a.get("migrate") {
        Action::Migrate {
            clade_id: num(&m["clade_id"])? as u32,
            from: point(&m["from"])?,
            to: point(&m["to"])?,
        }
    } else if let Some(r) = a.get("revive") {
        let steps = r["steps"]
            .as_array()
            .map(|s| s.iter().map(point).collect::<Result<Vec<_>, _>>())
            .transpose()?
            .unwrap_or_default();
        Action::Revive {
            source: if r["museum"].as_bool().unwrap_or(true) {
                Source::Museum
            } else {
                Source::SporeBank
            },
            entry_id: num(&r["entry_id"])? as u32,
            steps,
            at: point(&r["at"])?,
        }
    } else {
        return Err("no action".into());
    };
    let w = Wish {
        world_id: hex_arr(&req["world_id"])?,
        ruleset_id: hex_arr(&req["ruleset_id"])?,
        action,
        author: hex_arr(&req["author"])?,
        created_epoch: num(&req["created_epoch"])?,
        expires_epoch: num(&req["expires_epoch"])?,
        hypothesis: None,
        name: None,
    };
    w.check().map_err(|e| format!("{e:?}"))?;
    Ok(w)
}

fn spark_op(req: &serde_json::Value) -> Result<serde_json::Value, String> {
    use protogaea_protocol::spark::Spark;
    use protogaea_protocol::sth::{Receipt, Sth};
    use protogaea_protocol::wish;
    match req["op"].as_str().unwrap_or("") {
        "public" => Ok(json!({ "public": hex(&wish::public_key(&hex_arr(&req["secret"])?)) })),
        "wish" => {
            let w = wish_of(req)?;
            let id = w.id();
            let signature = wish::sign(&hex_arr(&req["secret"])?, &id);
            Ok(json!({ "bytes": hex(&w.to_bytes()), "id": hex(&id), "signature": hex(&signature) }))
        }
        "mine" => {
            // Tries `count` nonces from `nonce`; returns the sparks found and the next nonce.
            let world_id: [u8; 16] = hex_arr(&req["world_id"])?;
            let epoch = num(&req["epoch"])?;
            let challenge: [u8; 32] = hex_arr(&req["challenge"])?;
            let target = num(&req["target"])?;
            let proposal_id: [u8; 32] = hex_arr(&req["proposal_id"])?;
            let miner: [u8; 32] = hex_arr(&req["miner"])?;
            let (mut nonce, count) = (num(&req["nonce"])?, num(&req["count"])?);
            let mut found = Vec::new();
            SPARK_HASHER.with(|h| {
                let mut h = h.borrow_mut();
                let hasher =
                    h.get_or_insert_with(|| protogaea_pow::Hasher::new(protogaea_pow::SPARK));
                for _ in 0..count {
                    let s = Spark {
                        proposal_id,
                        miner,
                        nonce,
                    };
                    if s.check(hasher, &world_id, epoch, &challenge, target)
                        .is_some()
                    {
                        found.push(hex(&s.to_bytes()));
                    }
                    nonce = nonce.wrapping_add(1);
                }
            });
            Ok(json!({ "found": found, "next": nonce.to_string() }))
        }
        "receipt" => {
            let r = &req["receipt"];
            let s = &r["sth"];
            let path = r["path"]
                .as_array()
                .ok_or("no path")?
                .iter()
                .map(hex_arr::<32>)
                .collect::<Result<Vec<_>, _>>()?;
            let receipt = Receipt {
                spark: Spark::from_bytes(&hex_arr::<72>(&req["spark"])?),
                leaf_index: num(&r["leaf_index"])?,
                sth: Sth {
                    epoch: num(&s["epoch"])?,
                    tree_size: num(&s["tree_size"])?,
                    root: hex_arr(&s["root"])?,
                    timestamp_ms: num(&s["timestamp_ms"])?,
                    signature: hex_arr(&s["signature"])?,
                },
                path,
            };
            Ok(json!({ "ok": receipt.verify(&hex_arr(&req["operator"])?) }))
        }
        other => Err(format!("no such operation: {other}")),
    }
}

/// The spark client's operations, JSON in and out: `public`, `wish`, `mine`, `receipt`. Returns
/// 0 with the answer in the output, or -1 with the error.
///
/// # Safety
/// `ptr` must point to `len` initialized bytes.
#[no_mangle]
pub unsafe extern "C" fn spark_api(ptr: *const u8, len: usize) -> i32 {
    let result = serde_json::from_slice::<serde_json::Value>(std::slice::from_raw_parts(ptr, len))
        .map_err(|e| e.to_string())
        .and_then(|req| spark_op(&req));
    match result {
        Ok(v) => {
            put(serde_json::to_vec(&v).expect("JSON"));
            0
        }
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

    /// The spark client's operations: a signed wish that the protocol accepts, mining that finds
    /// sparks under an easy target, and a receipt checked against the operator's key.
    #[test]
    fn spark_api_operations() {
        use protogaea_protocol::{log, spark::Spark, sth::Sth, wish};
        let secret = [5u8; 32];
        let author = hex(&wish::public_key(&secret));
        let req = json!({
            "op": "wish", "secret": hex(&secret), "author": author,
            "world_id": hex(&[1u8; 16]), "ruleset_id": hex(&[2u8; 32]),
            "created_epoch": 10, "expires_epoch": 298,
            "action": { "weather": { "x": 3, "y": 4, "rain": true } },
        });
        let w = spark_op(&req).unwrap();
        let bytes: Vec<u8> = (0..w["bytes"].as_str().unwrap().len() / 2)
            .map(|i| {
                u8::from_str_radix(&w["bytes"].as_str().unwrap()[2 * i..2 * i + 2], 16).unwrap()
            })
            .collect();
        let parsed = wish::Wish::from_bytes(&bytes).unwrap();
        let sig: [u8; 64] = unhex_n(w["signature"].as_str().unwrap()).unwrap();
        assert_eq!(wish::verify(&parsed.author, &parsed.id(), &sig), Ok(()));

        let mine = spark_op(&json!({
            "op": "mine", "world_id": hex(&[1u8; 16]), "epoch": 10, "challenge": hex(&[3u8; 32]),
            "target": (u64::MAX / 4).to_string(), "proposal_id": w["id"], "miner": author,
            "nonce": "0", "count": 20,
        }))
        .unwrap();
        let found = mine["found"].as_array().unwrap();
        assert!(!found.is_empty(), "a quarter of hashes are sparks");
        assert_eq!(mine["next"], "20");

        let spark_hex = found[0].as_str().unwrap();
        let spark = Spark::from_bytes(&unhex_n::<72>(spark_hex).unwrap());
        let operator = [9u8; 32];
        let leaves = vec![log::leaf_hash(&spark.leaf(10))];
        let sth = Sth::sign(&operator, 10, 1, log::root(&leaves), 7);
        let receipt = json!({
            "leaf_index": 0, "path": [],
            "sth": { "epoch": 10, "tree_size": 1, "root": hex(&sth.root), "timestamp_ms": 7, "signature": hex(&sth.signature) },
        });
        let ok = spark_op(&json!({ "op": "receipt", "receipt": receipt, "spark": spark_hex, "operator": hex(&wish::public_key(&operator)) })).unwrap();
        assert_eq!(ok["ok"], true);
        let other = spark_op(
            &json!({ "op": "receipt", "receipt": receipt, "spark": spark_hex, "operator": author }),
        )
        .unwrap();
        assert_eq!(other["ok"], false);
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
