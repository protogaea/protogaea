# World rules: `ruleset.json`

> **Status: Draft.** The rules themselves are defined in [Part II of the specification](spec/spec-v0.2.md#part-ii-world-rules-ruleset-v0). This document describes the structure of the season rules file and lists every parameter with its candidate value, or **TBD** where the balance harness will set it (stage A).

## 1. Principles

- One file per season. `ruleset_id` is the BLAKE3 hash of the file's canonical encoding. The canonicalization is TBD; candidate: the JSON Canonicalization Scheme (RFC 8785).
- Integers only. Percentages are integers, probabilities are parts per million per epoch, energy is in hundredths of a unit.
- The file is immutable once the season starts. Any change means a new season and a new `ruleset_id`.
- The server, the replay and the browser use the same file. The client never uses hidden constants.

## 2. Structure

The top-level sections, with a few representative values. The layout is illustrative.

```json
{
  "version": 0,
  "season": { "number": 1, "title": "The Breaking of Pangea", "length_epochs": 12096 },
  "world": { "width": 64, "height": 64, "wrap": false,
             "ticks_per_epoch": 12, "epochs_per_day": 288, "epochs_per_year": 372 },
  "capacity": { "max_organisms": 6000, "max_per_cell": 4 },
  "genesis": { "seed": "TBD", "founder_lineages": 6, "organisms_per_lineage": 50 },
  "biomes": {},
  "times_of_year": {},
  "moisture": {},
  "rifts": {},
  "natural_events": {},
  "genome": {},
  "energy": {},
  "feeding": {},
  "hunting": {},
  "movement": {},
  "reproduction": {},
  "aging": {},
  "decomposition": {},
  "clades": {},
  "museum": {},
  "spore_bank": {},
  "actions": {},
  "ledger": {}
}
```

## 3. Parameters

### World, time and capacity

| Parameter | Value |
|---|---|
| Grid | 64 × 64, no wrap-around |
| Ticks per epoch | 12 |
| Epochs per world day | 288 (3,456 ticks) |
| Epochs per world year | 372 (4,464 ticks, 31 hours); four times of year of 93 epochs each |
| Season 1 length | 12,096 epochs (42 world days) |
| Capacity | 6,000 organisms; 4 per cell |
| Start | 6 founder lineages × 50 organisms, different archetypes, different biomes |

### Biomes and food

| Parameter | Value |
|---|---|
| Land biomes | forest, steppe, desert, mountains, swamp |
| Water | shallows (passable), deep water (impassable) |
| `base_regen[biome]`, `food_max[biome]` | TBD |
| `move_cost[biome]` | TBD; mountains and swamps cost more |

Food growth per tick:

```
regen = base_regen[biome] × season_mult[biome][season] / 100 × moisture_mult(moisture) / 100 + decomposition
```

`season` here means the time of year.

### Times of year: `season_mult` (candidate, %)

| Biome | Spring | Summer | Autumn | Winter |
|---|---:|---:|---:|---:|
| Forest | 110 | 120 | 90 | 40 |
| Steppe | 120 | 90 | 80 | 30 |
| Desert | 80 | 40 | 70 | 60 |
| Mountains | 70 | 100 | 60 | 20 |
| Swamp | 120 | 110 | 100 | 50 |

Transitions between times of year are smoothed tick by tick.

### Moisture

| Parameter | Value |
|---|---|
| Range | 0–100 |
| `moisture_relax` | TBD — per-tick return toward the biome's base level |
| `moisture_mult` | A table from 50% at 0 to 110% at 100 (candidate) |

### Rifts (Season 1)

| Cell phase | Food growth | Step cost | Passable |
|---|---|---|---|
| Crack | as the biome | as the biome | yes |
| Fault | ×0.5 | ×2 | yes |
| Shallows | 0 | ×3, plus `shallow_drain` (TBD) at the end of a tick | yes |
| Deep water | 0 | — | no |

The schedule by world day: unity 0–6, cracks 7–13, shallows 14–23, straits 24–34, the last bridges 35–38, continents 39–42. Rift lines are generated from `genesis_seed`. Organisms in a cell that becomes deep water move to the nearest free land, or drown.

### Natural events

| Event | Conditions | Effect | Frequency (candidate) |
|---|---|---|---|
| Wildfire | forest or steppe, summer, moisture < 30 | radius 2–4: food and detritus → 0, organisms −50% energy; then ash: growth +50% for 72 ticks | once every 2–3 world days |
| Flood | swamp and cells next to water, spring | cells become shallows for 24 ticks | once a day in spring |
| Great drought | steppe and desert, summer | 9 × 9: growth ×0.5 for 72 ticks | once every 3 days |
| Plague | a clade denser than a threshold (TBD) | mortality `plague_p` (TBD) within radius 3; the event's probability grows with the clade's share of the world | by dominance |

Probabilities are stored in parts per million per epoch (TBD). At most two events of each type are active at once.

### Genome

| Gene | Range | Phenotype |
|---|---|---|
| `M` movement | 0–8 | steps per tick = `(M + 2) / 3` |
| `P` perception | 0–8 | sight radius = `1 + P / 3` |
| `G` plant eating | 0–8 | bite = `G × bite_per_point` |
| `H` hunting | 0–8 | attack strength; 0 means no hunting |
| `D` defense | 0–8 | defense strength |
| `F` fertility | 0–8 | lowers the reproduction threshold and the offspring's energy |
| `habitat` | 0–5 | preferred land biome (0–4): metabolism −10% there, +10% elsewhere; 5 = generalist |
| `dispersal` | 0–3 | tendency to leave settled cells and relatives |
| `boldness` | 0–3 | how much food outweighs danger |
| `hue` | 0–359 | neutral color; no effect |

| Mutation parameter | Value |
|---|---|
| `mutation_rate` | TBD — one step between a valid pair of traits |
| `behavior_mutation_rate` | TBD |
| `hue_mutation_rate` | TBD — shift of ±1…8 |

### Energy, feeding, hunting

| Parameter | Value |
|---|---|
| Energy unit | hundredths, stored as an integer |
| `base_metabolism`, `trait_upkeep[6]`, `energy_max` | TBD; hunting, defense and movement cost more than plant eating and fertility |
| `habitat_bonus` | ±10% of metabolism |
| `bite_per_point`, `plant_efficiency` | TBD |
| `attack_weight`, `defense_weight`, `roll_span`, `attack_cost` | TBD |
| `predation_efficiency`, `body_value` | TBD |
| `kin_distance` | 2 — no attacks on organisms within this genome distance |

Attack and defense:

```
attack  = H × attack_weight  + P             + rand(0 … roll_span)
defense = D × defense_weight + M + P_prey / 2 + rand(0 … roll_span)
```

### Movement choice

```
score(c) = w_food × expected_food(c) + w_hunt × prey_vulnerability(c)
         − w_danger × threat(c) × (4 − boldness) + w_habitat × [biome(c) = habitat]
         − w_crowd × occupancy(c) − w_move × path_cost(c)
         + w_disp × dispersal × distance_from_kin(c)
```

The weights `w_*` are TBD. Ties are broken by counter-based randomness.

### Reproduction, aging, decomposition

| Parameter | Value |
|---|---|
| `repro_base`, `repro_per_F` | TBD; threshold = `repro_base − F × repro_per_F` |
| `child_base`, `child_per_F` | TBD; offspring energy = `child_base − F × child_per_F` |
| `birth_cost` | TBD |
| `senescence_start` | 150 ticks |
| `max_age` | 300 ticks |
| `detritus_share`, `decomposition_rate` | TBD |

### Clades, museum and spore bank

| Parameter | Value |
|---|---|
| `clade_split_distance` | 3 steps |
| `clade_name_threshold` | 20 living organisms |
| Museum capacity in the state | 1,024 extinct named clades |
| Natural revival | fewer than 30 living organisms → 5 per genesis genome; at most once every 288 epochs |
| Season ends by extinction | 3 natural revivals within 7 world days |

### Actions

| Action | Parameters |
|---|---|
| `weather` | 7 × 7 area, 36 ticks, growth ×1.5 (`rain`) or ×0.5 (`drought`), moisture ±30, region cooldown 12 epochs |
| `migrate` | 5 × 5 source area, at least 10 organisms of the clade, up to 3 moved, clade cooldown 12 epochs; crossing water is allowed |
| `revive` | at most 2 mutation steps; museum entry extinct for at least 36 epochs; at most 8 organisms in the surrounding 5 × 5; each entry at most once per world day; 5 organisms in a 3 × 3 area |
| `ring` | at most 20 per key per day; not part of the state |

Order at the epoch boundary: rifts → natural events → `weather` → `migrate` → `revive` → natural revival.

### Ledger

| Parameter | Value |
|---|---|
| Miracles per epoch | at most 3 |
| `price_mult` | `weather` 100, `migrate` 120, `revive` 200 (candidates) |
| `P_min` | ≈ 8 core-hours of the reference core; TBD in work units |
| Price step | ±1/8 per epoch |
| Wish lifetime | at most 288 epochs |
| Open wishes per key | at most 3 |
| Spark target | about 20,000 sparks per epoch; step at most ±25% |

## 4. Invariants

The core checks these on every tick; a violation is a consensus error that halts the season.

- The six traits sum to 24, and each lies in 0–8. Behavioral genes and `hue` stay within their ranges.
- At most 4 organisms per cell and at most 6,000 in total.
- Energy never exceeds `energy_max`; an organism with energy ≤ 0 dies.
- IDs strictly increase; `next_organism_id` and `next_clade_id` never go back.
- All values are integers. No floating point, no system time, no hash-table iteration.

## 5. Order of operations

See [spec §13](spec/spec-v0.2.md#13-epoch-and-tick-order) for the epoch boundary and the six steps of a tick.

## 6. Open items

- Every TBD number above, to be set by the balance harness against the metrics in [spec §28](spec/spec-v0.2.md#28-ecosystem-health-the-balance-harness).
- The canonical encoding of the file and the exact definition of `ruleset_id`.
- The plague density threshold and the table form of `moisture_mult`.
