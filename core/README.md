# protogaea-core

The deterministic simulation core of Protogaea — consensus code. The same inputs must produce the same bytes on every platform, so this crate follows the rules of [spec §14](../docs/spec/spec-v0.2.md#14-determinism-and-randomness):

- integers only — no floating point anywhere;
- counter-based randomness, with no generator state;
- no iteration over hash tables — clades live in a `BTreeMap`, organisms in a vector ordered by id;
- overflow checks stay on in release builds (`Cargo.toml`), so overflow never wraps silently;
- no system time.

CI builds the crate for `wasm32-unknown-unknown` and compares state hashes across x86-64, ARM64, Windows, macOS and WASM on every push.

## Status against the specification (stage A1)

| Area | Status |
|---|---|
| Counter-based randomness (§14) | Done. `BLAKE3("PROTOGAEA/RAND/V0" ‖ seed ‖ tick: u32 ‖ subject: u64 ‖ purpose: u32 ‖ k: u32)`, little-endian; uniform draws by rejection sampling |
| Epoch seed (§14) | Done. The harness supplies a stand-in beacon until stage C |
| Map (§9) | A single continent: five land biomes, coastal shallows, deep water. Founders come from the ruleset |
| Genome and mutations (§11.1) | Done |
| Energy and feeding (§11.2) | Done |
| Hunting (§11.3) | Done, including satiation and cover |
| Movement choice (§11.4) | Simplified (see below) |
| Reproduction (§11.5) | Done |
| Death and decomposition (§11.6) | Done |
| Clades (§12) | Done. The museum and the spore bank arrive in stage A2 |
| Tick order (§13) | Done. The epoch boundary draws natural events; rifts, miracles and natural revival are still to come |
| Times of year and moisture (§10) | Done: a 372-epoch year, smooth transitions, moisture drifting to a seasonal target |
| Natural events (§10) | Wildfire with ash, great drought, plague ("kill the winner"). Floods arrive with rifts |
| Rifts (§10) | Still to come in stage A2 |
| State root (§15) | A flat BLAKE3 hash of the canonical encoding; the Merkle root arrives in stage A2 |
| `ruleset_id` | BLAKE3 over serde's JSON encoding; the final canonicalization is TBD |

## Differences from the specification

To be resolved in the next version of the specification.

1. **Movement score approximations** (§11.4):
   - `prey_vulnerability(c)` uses the weakest organism in the cell at the start of the tick;
   - `threat(c)` uses the strongest hunter in the 3 × 3 area around the cell;
   - the kin check in both uses the clade instead of the genome distance. The attack itself uses the exact genome distance (`kin_distance`).
2. **Dispersal** is approximated as a bonus per step of distance (`dispersal_per_step`), not as distance from relatives.
3. **Seasonal moisture** (`season_moisture_delta`): the base moisture shifts with the time of year (drier in summer), so that summer wildfires are possible.
4. **Food units** are tenths of a unit, so that integer multipliers do not round slow growth down to zero.

## Layout

| File | Contents |
|---|---|
| `rng.rs` | Counter-based randomness and seed derivation |
| `genome.rs` | The genome, phenotype and mutations |
| `ruleset.rs` | Parameters and their validation |
| `state.rs` | The world state, its canonical encoding, hash and invariants |
| `map.rs` | Terrain generation and genesis |
| `sim.rs` | One epoch: the tick in six steps |
