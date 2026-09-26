# protogaea-core

The deterministic simulation core of Protogaea — consensus code. The same inputs must produce the same bytes on every platform, so this crate follows the rules of [spec §14](../docs/spec/spec-v0.2.md#14-determinism-and-randomness):

- integers only — no floating point anywhere;
- counter-based randomness, with no generator state;
- no iteration over hash tables — clades live in a `BTreeMap`, organisms in a vector ordered by id;
- overflow checks stay on in release builds (`Cargo.toml`), so overflow never wraps silently;
- no system time.

CI builds the crate for `wasm32-unknown-unknown` and compares state roots across x86-64, ARM64, Windows, macOS and WASM on every push.

## The state root

`World::state_root()` hashes the state as one Merkle tree per kind of data, shaped as in RFC 9162 with BLAKE3: a leaf is `H(0x00 ‖ domain ‖ bytes)`, a node `H(0x01 ‖ left ‖ right)`, and a tree of `n` leaves splits at the largest power of two below `n`, so no leaf is duplicated. An empty tree is `H("PROTOGAEA/MERKLE/EMPTY/" ‖ domain)`.

| Subtree | Leaves, in order | Domain |
|---|---|---|
| globals | one leaf: world id, `ruleset_id`, map size, epoch, `next_organism_id`, `next_clade_id`, `ended`, and the length of every list below | `PROTOGAEA/STATE/A3/GLOBALS` |
| cells | every cell by index; the index is part of the leaf | `…/CELL` |
| organisms | living organisms by id | `…/ORGANISM` |
| clades | living clades by id | `…/CLADE` |
| museum | extinct named clades, oldest first | `…/MUSEUM` |
| effects | active effects, in the order they started | `…/EFFECT` |
| rifts | the rift schedule by cell | `…/RIFT` |
| spore bank | its genomes | `…/SPORE` |
| revivals | the epochs of natural revivals | `…/REVIVAL` |

`state_root = BLAKE3("PROTOGAEA/STATE_ROOT/A3" ‖ globals ‖ cells ‖ organisms ‖ clades ‖ museum ‖ effects ‖ rifts ‖ spore bank ‖ revivals)`. The same value seeds the next epoch as the previous state. `World::prove_organism(id)` returns an organism with its audit path and the other subtree roots; `OrganismProof::verify` checks it against a published `state_root` without the rest of the state. Computing the root costs about 3.5 ms per epoch on the reference machine (harness findings).

## Status against the specification (stages A1–A3)

| Area | Status |
|---|---|
| Counter-based randomness (§14) | Done. `BLAKE3("PROTOGAEA/RAND/V0" ‖ seed ‖ tick: u32 ‖ subject: u64 ‖ purpose: u32 ‖ k: u32)`, little-endian; uniform draws by rejection sampling |
| Epoch seed (§14) | Done. The harness supplies a stand-in beacon until stage C |
| Map (§9) | A single continent: five land biomes, coastal shallows, deep water. Founders come from the ruleset |
| The Breaking of Pangea (§4, §10) | Done: 3–4 plates with bent boundaries, rift lines, waves of flooding from the ocean inward, one land bridge per pair of neighboring plates, organisms carried off sinking cells or drowned. The schedule lives in the state |
| Genome and mutations (§11.1) | Done |
| Energy and feeding (§11.2) | Done |
| Hunting (§11.3) | Done, including satiation and cover |
| Movement choice (§11.4) | Simplified (see below) |
| Reproduction (§11.5) | Done |
| Death and decomposition (§11.6) | Done |
| Clades (§12) | Done, with the museum of extinct named clades |
| Spore bank (§12) | Done: natural revival and the end of a season by extinction |
| Tick order (§13) | Done. The epoch boundary applies the rift schedule, draws natural events, applies miracles (`weather`, `migrate`, `revive`, with their hard checks and cooldowns) and runs natural revival |
| Times of year and moisture (§10) | Done: a 372-epoch year, smooth transitions, moisture drifting to a seasonal target |
| Natural events (§10) | Floods, wildfire with ash, great drought, plague ("kill the winner") |
| State root (§15) | Done: Merkle trees in the shape of RFC 9162 (see below), with inclusion proofs for organisms |
| `ruleset_id` | BLAKE3 over serde's JSON encoding; the final canonicalization is TBD |

## Differences from the specification

To be resolved in the next version of the specification.

1. **Movement score approximations** (§11.4):
   - `prey_vulnerability(c)` uses the weakest organism in the cell at the start of the tick;
   - `threat(c)` uses the strongest hunter in the 3 × 3 area around the cell;
   - the kin check in both uses the clade instead of the genome distance. The attack itself uses the exact genome distance (`kin_distance`).
2. **Dispersal** is approximated as a bonus per step of distance (`dispersal_per_step`), not as distance from relatives.
3. **Food units** are tenths of a unit, so that integer multipliers do not round slow growth down to zero.

## Layout

| File | Contents |
|---|---|
| `rng.rs` | Counter-based randomness and seed derivation |
| `genome.rs` | The genome, phenotype and mutations |
| `ruleset.rs` | Parameters and their validation |
| `state.rs` | The world state, its canonical encoding, `state_root`, organism proofs and invariants |
| `merkle.rs` | Merkle trees and inclusion proofs (RFC 9162 shape) |
| `map.rs` | Terrain generation and genesis |
| `rifts.rs` | Plates, rift lines, land bridges and their schedule |
| `climate.rs` | Times of year and moisture |
| `sim.rs` | One epoch: the tick in six steps |
