# World rules: `ruleset.json`

> **Status: Draft.** The rules themselves are defined in [Part II of the specification](spec/spec-v0.2.md#part-ii-world-rules-ruleset-v0). This document describes the structure of the season rules file and lists every parameter with its candidate value, or **TBD** where the balance harness will set it (stage A).

**Stage A implementation.** [core/src/ruleset.rs](../core/src/ruleset.rs) holds the parameters the core uses, with untuned default values; `protogaea-harness ruleset` prints them as JSON. Satiation (`hunt_hunger_pct`) and cover (`cover`) were added to spec §11.3 during stage A1. The remaining differences from the specification are listed in [core/README.md](../core/README.md#differences-from-the-specification).

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
| Start | 6 founder lineages × 50 organisms, different archetypes, different biomes; the lineages go to the plates in turn and settle away from the future rifts, so every future continent starts with founders |

### Biomes and food

| Parameter | Value |
|---|---|
| Land biomes | forest, steppe, desert, mountains, swamp |
| Water | shallows (passable), deep water (impassable) |
| `base_regen[biome]`, `food_max[biome]` | TBD |
| `move_cost[biome]` | TBD; mountains and swamps cost more |
| `biome_mix` | 12% of the land is mountains (the highest cells); the rest by moisture, dry to wet: 20% desert, 30% steppe, 35% forest, 15% swamp |

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

Each value holds at the midpoint of its time of year; between midpoints it changes linearly tick by tick, in integers. The world starts at the beginning of spring.

### Moisture

| Parameter | Value |
|---|---|
| Range | 0–100 |
| `moisture_base[biome]` | forest 70, steppe 45, desert 15, mountains 50, swamp 90, water 100 |
| `season_moisture_delta` | spring +10, summer −20, autumn 0, winter +10; follows the times of year like `season_mult` |
| `moisture_relax` | 1 per tick toward `moisture_base + season_moisture_delta`, limited to 0–100 |
| `moisture_mult` | linear from `moisture_mult_min_pct` (50) at 0 to `moisture_mult_max_pct` (110) at 100 |
| `cold_winter_pct` | 0 — optional cold: extra energy cost of every organism in midwinter at the northern edge, following the times of year and falling linearly to zero at the southern edge |

### Rifts (Season 1)

| Cell phase | Food growth | Step cost | Passable |
|---|---|---|---|
| Crack | as the biome | as the biome | yes |
| Fault | ×0.5 | ×2 | yes |
| Shallows | 0 | ×3, plus `shallow_drain` (TBD) at the end of a tick | yes |
| Deep water | 0 | — | no |

The schedule by world day: unity 0–6, cracks 7–13, shallows 14–23, straits 24–34, the last bridges 35–38, continents 39–42. The generator is described in [spec §10](spec/spec-v0.2.md#10-times-of-year-climate-rifts-and-natural-events).

| Parameter | Value |
|---|---|
| `plates_min`, `plates_max` | 3, 4 — the number of plates (future continents), drawn at genesis |
| `boundary_warp` | 5 cells — how far noise bends the plate boundaries |
| `fault_day` | 7 |
| `shallows_from_day`, `deep_from_day` | 14, 24 — waves from the ocean inward |
| `bridges_from_day`, `bridges_to_day` | 35, 39 — land bridges close one by one |
| `fault_growth_pct`, `fault_move_pct` | 50, 200 |
| `bridge_radius` | 2 — a bridge is the rift cells within this distance of its center |
| `rescue_radius` | 8 — organisms on a sinking cell move to the nearest free land within this distance, or drown |
| `plate_mixes` | four biome mixes, one per plate in turn from a random start, so that the future continents differ; each keeps all five land biomes (mountains % of the land; desert / steppe / forest % of the rest, swamp the remainder): arid steppe 10; 32/44/16, forest 10; 6/18/56, highland 28; 16/30/36, wetland 8; 8/22/34. An empty list means `biome_mix` everywhere |

### Natural events

| Event | Conditions | Effect | Frequency (candidate) |
|---|---|---|---|
| Wildfire | forest or steppe, summer, moisture < 30 | radius 2–4: food and detritus → 0, organisms −50% energy; then ash: growth +50% for 72 ticks | once every 2–3 world days |
| Flood | swamp and land next to water, spring | land other than mountains within radius 2 acts as shallows for 24 ticks; its food is lost and its moisture set to 100 | once a day in spring (3,472 ppm per spring epoch) |
| Great drought | steppe and desert, summer | 9 × 9: moisture −30 at once, growth ×0.5 for 72 ticks | once every 3 days |
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

The six traits `M`–`F` always sum to `trait_budget` (24), so one grows only at another's expense.

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
| `kin_distance` | 1 — no attacks on organisms within this genome distance (2 until the Season 1 tuning) |
| `hunt_hunger_pct` | 70 — hunting only while energy is below this share of `energy_max` (60 until the Season 1 tuning) |
| `cover` | forest 12, swamp 10, mountains 14, open land 0 — a defense bonus in the biome |

Attack and defense:

```
attack  = H × attack_weight  + P             + rand(0 … roll_span)
defense = D × defense_weight + M + P_prey / 2 + cover + rand(0 … roll_span)
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
| Natural revival | fewer than 30 living organisms → 5 per genesis genome in free cells of its starting biome, each genome founding a new clade; at most once every 288 epochs |
| Season ends by extinction | 3 natural revivals within 7 world days |

### Actions

| Action | Parameters |
|---|---|
| `weather` | 7 × 7 area, 36 ticks, growth ×1.5 (`rain`) or ×0.5 (`drought`), moisture ±30, region cooldown 12 epochs |
| `migrate` | 5 × 5 source area, at least 10 organisms of the clade, up to 3 moved, clade cooldown 12 epochs; crossing water is allowed |
| `revive` | at most 2 mutation steps; museum entry extinct for at least 36 epochs; at most 8 organisms in the surrounding 5 × 5; each entry at most once per world day; 5 organisms in a 3 × 3 area |
| `ring` | at most 20 per key per day; not part of the state |

Order at the epoch boundary: rifts → natural events → `weather` → `migrate` → `revive` → natural revival.

**In the core** ([`core/src/miracle.rs`](../core/src/miracle.rs), parameters in `Ruleset::miracles`): each miracle is checked hard when applied and refused with a reason if it no longer holds; the same check is the soft check of open wishes. `weather` changes moisture once when it begins and food growth for its 36 ticks; its region is the 7 × 7 square, and two regions overlap if their centers are within 6 cells. `migrate` moves the clade's lowest ids in the source area into the 3 × 3 around the target, filling cells up to `max_per_cell` in row order. `revive` places 5 organisms in the 3 × 3 around the start the same way; each edit step moves one point from trait `i` to trait `j`, so the trait budget holds; a museum revival's clade has the museum clade as its parent, a spore bank revival's has none. Cooldowns are part of the state; their subtree enters `state_root` only while one runs, so a world without miracles keeps its roots.

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
