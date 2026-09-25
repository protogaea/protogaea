# protogaea-harness

The balance harness ([spec §28](../docs/spec/spec-v0.2.md#28-ecosystem-health-the-balance-harness)): it runs worlds offline, measures their ecosystem health and draws reports. It is not consensus code.

## Usage

```sh
# One world: progress, then runs/seed-1/{metrics.csv, summary.json, ruleset.json, report.html}
cargo run --release -p protogaea-harness -- run --seed 1 --days 3

# Many worlds in parallel: the ecosystem health checks for each seed, and pass rates
cargo run --release -p protogaea-harness -- sweep --seeds 1..17 --days 2 --out sweep.csv

# State hashes at checkpoints, for cross-platform determinism checks (used by CI)
cargo run --release -p protogaea-harness -- hash --seeds 1,2 --epochs 500 --every 100

# The default ruleset as JSON; edit it and pass it back with --ruleset
cargo run --release -p protogaea-harness -- ruleset > my-rules.json

# The Season 1 map criteria of spec §4 for candidate seeds, without running them
cargo run --release -p protogaea-harness -- maps --seeds 1..21

# Performance on one thread: epoch times over a world day after two days of warm-up
cargo run --release -p protogaea-harness -- bench --seed 1 --warmup 2 --days 1
# The same under WASI
cargo build --release -p protogaea-harness --target wasm32-wasip1
wasmtime run target/wasm32-wasip1/release/protogaea-harness.wasm bench --seed 1
```

`report.html` is self-contained: open it in a browser to watch the map, the population by type, the Muller plot of clade shares and the mean traits. The map follows the Breaking of Pangea: rift lines, faults, flooding and sinking rifts, land bridges until they close, and floods, ash and droughts as tints.

`maps` checks the criteria a Season 1 seed must meet (spec §4): 3–5 land bridges in different biomes, every future continent keeping all five land biomes after the breakup, and at least 40 founders starting on each of them. The season seed is chosen from at least 20 candidates.

The beacon is replaced by a value derived from the run seed, so every run is reproducible.

## Live mode

A world running in real time, as a preview before the stage B viewer:

```sh
PROTOGAEA_PASSWORD=choose-one cargo run --release -p protogaea-harness -- live --seed 5
```

- One epoch every `--epoch-seconds` (300 by default: world time runs at the pace of real time). After a pause the world does not catch up.
- The page at `http://127.0.0.1:8080` (`--listen`) is the run report for the last seven world days, opened at the latest map frame. Reload it to update.
- `PROTOGAEA_PASSWORD`, and optionally `PROTOGAEA_USER` (by default `protogaea`), put the page behind HTTP Basic authentication. Without a password anyone who can reach the port sees the page. `/health` is always open.
- A snapshot goes to `--data` (`runs/live`) every `--snapshot-every` epochs (12). On start the world resumes from it, with the saved seed and ruleset, and continues exactly as if it had never stopped.
- The page is served over plain HTTP, so the password crosses the network unencrypted: it is a lock for tests, not protection. Anything more needs TLS in front.

[deploy/protogaea-live.service](../deploy/protogaea-live.service) runs live mode as a systemd service with resource limits and sandboxing.

## Static Linux build

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl -p protogaea-harness
```

The result is a single static binary. [.cargo/config.toml](../.cargo/config.toml) links it with `rust-lld`, and BLAKE3 uses its portable Rust implementation, so no C toolchain is needed, even on Windows.

## Checks

These are the checks of spec §28 measured so far. The thresholds are candidates.

- no total extinction: the population never falls to zero;
- predators survive to the end;
- the spore bank revives the world at most once;
- at least 6 clades of 20+ organisms after day 3;
- the dominant clade changes at least once per 3 days;
- no clade holds more than 60% of the population for more than 3 days;
- under 1% of ticks at the global limit;
- equilibrium population at 40–70% of the limit;
- at least 30 generations per world day;
- each plate ends as its own continent (judged once the last land bridge has closed);
- each continent ends with its own fauna: the clade makeup of different continents is at least 80% apart and their mean hues at least 30° apart at the end of the season (judged on runs of a full season).

## Findings (stage A1)

Measured on 2026-09-25 with 16 seeds.

- **The world lives.** With the default rules the population settles at about 3,000 organisms (about 50% of the limit), with more than 30 generations per world day and no ticks at the global limit.
- **Predators overexploit and die out.** With the initial rules (no cover, a dispersal bonus of 50), hunters wipe out the grazers within 2–3 epochs, then starve among armored organisms they cannot beat. They die out on 16 of 16 seeds.
- **Satiation alone does not help** (0 of 16 seeds): a fed hunter breeds and is hungry again.
- **Smaller gains from prey help little:** predators survive on at most 25% of seeds.
- **Cover works together with a small dispersal bonus.** With cover of +12 in forests, +10 in swamps and +14 in mountains, and a dispersal bonus of 10 per step instead of 50, predators survive one day on 13 of 16 seeds and three days on 14 of 16.
- **Still open:** in most seeds a single clade holds more than 60% of the population for almost the whole three days.

Satiation and cover were adopted into the specification (§11.3) on 2026-09-25. The default ruleset now includes cover and the dispersal bonus of 10.

## Findings (stage A2, part 1: times of year and natural events)

Measured on 2026-09-25 with 16 seeds.

- **Seasons drive the population:** about 4,200 organisms in spring, 1,800–2,100 in winter. Dominance shifts over the year: armored organisms give way to grazers.
- **Diversity improved:** at least 6 clades of 20+ after day 3 on 87% of seeds (4 days), and no clade above 60% for more than 3 days on 81%.
- **Winter killed the predators** on about half of the seeds: pure carnivores starve when prey is scarce and hides in cover.
- **Cheaper hunting upkeep fixed it** (10 instead of 15 per point): predators survive on 16 of 16 seeds over 4 days. An omnivorous founder hunter did not help.
- **Wildfires were too rare:** they need dry steppe or forest late in summer, so the chance per summer epoch was raised from 5,556 to 9,000 ppm.

With these defaults, over 8 world days: no extinction and predators alive on 16 of 16 seeds; at least 6 clades of 20+ after day 3 and a change of the dominant clade at least once per 3 days on 68%; no clade above 60% for more than 3 days on 87%; all 8 checks pass on 9 of 16 seeds. Next: more turnover of the dominant clade (stage A3 tuning).

## Findings (stage A2, part 2: the Breaking of Pangea)

Measured on 2026-09-25 with 20 seeds over a full 42-day season (12 seeds with 3 plates, 8 with 4).

- **The breakup works as designed.** On every seed each plate ends as its own continent. Rescue always found free land: nobody drowned.
- **The world stays healthy through the whole season:** no extinction, predators alive at the end, the spore bank never needed, equilibrium at 45–49% of the limit, 35–37 generations per world day and no ticks at the global limit on 20 of 20 seeds. The dominant clade changes at least once per 3 days, and no clade holds 60% for more than 3 days, on 19 of 20.
- **Diversity dips over a long season.** At least 6 clades of 20+ at every moment after day 3 holds on only 5 of 20 seeds; in some winters the count falls to 2–5. Over 8-day runs it held on 68%.
- **Continents do not diverge yet.** The divergence between the dominant clades of different continents grows by 3+ steps on 1 of 20 seeds; the median growth is 0.55 steps. Isolation is short (the last bridge closes on day 38), and every continent pulls toward the same trait optimum, because all of them share one climate and all five biomes.

`maps --seeds 1..21` finds 9 of 20 seeds that meet every map criterion of spec §4; the others miss the 40 founders per continent (founder lineages start in clusters) or have a continent without one of the biomes.

Next, stage A3: continents that differ enough to diverge (for example climates or biome mixes that vary by plate, or earlier isolation), and steadier diversity through the winters.

## Findings (stage A3: why the continents' traits do not diverge)

Measured on 2026-09-25 over full 42-day seasons, 12 seeds per variant (20 for the baseline). `run` now prints each plate's population, dominant clade and mean traits; `sweep` reports the growth of the distance between the plates' mean genomes (`cdiv+`), how different their clade makeup is at the start of phase III and at the end (`comp%→`, Bray–Curtis) and the angle between their mean hues (`hue°`).

- **One corner of the genome wins everywhere.** With a trait budget of 24 and a cap of 8, a genome can nearly max out plant eating, defense and fertility at once (G 7, D 7, F 8). Every plate converges on that armored breeder, and hunters stay at about 1% of the population.
- **A smaller budget makes the ecology healthier, not more divergent.** With `trait_budget` 18 or 16 (founders scaled down), hunters are 2–3× more numerous and no clade holds 60% for more than 3 days on 100% of seeds, but the plates' mean genomes still end within about one step of each other.
- **Different surroundings do not split the traits either.** Per-plate biome mixes (`plate_mixes`: arid steppe, forest, highland, wetland), earlier isolation (bridges closed by day 24 instead of 38) and a cold north (`cold_winter_pct` 60) all leave the median `cdiv+` below 1.2 steps. With the same rules everywhere, each continent settles into the same mix of strategies; a gap of 2–3 steps appears only for a while, when continents are in different phases of the grazer–armored–hunter cycle.
- **The continents' fauna does diverge.** The clade makeup of different plates differs by about 45% at the start of phase III and by 86–100% at the end of the season (no shared clade at all on most seeds with early isolation), and the plates' mean hues end 40–120° apart. Each continent ends with its own lineages and its own colors on the map.

Specification §28 was changed accordingly: the divergence check now asks for clade makeup at least 80% apart and hues at least 30° apart by the end of the season; the distance between mean traits is reported but not required. With the default rules it passes on 16 of 20 seeds: the hues always diverge, and on the other four the clade makeup ends 64–79% apart. `trait_budget` and `cold_winter_pct` stay in the ruleset as options for later seasons.

## Findings (stage A3: performance)

Measured on 2026-09-26 with `bench` on one thread of an AMD Ryzen 5 5500, default rules, after two world days of warm-up (population 1,500–4,800). Each epoch includes the step and the state hash.

| Build | Seed | World day | Epoch p50 | Epoch p95 | Epoch max |
|---|---|---|---|---|---|
| native (x86-64) | 1 | 11.8 s | 42.5 ms | 59.4 ms | 62.8 ms |
| native (x86-64) | 5 | 11.0 s | 39.7 ms | 53.7 ms | 57.4 ms |
| WASM (wasmtime 49) | 1 | 16.9 s | 61.4 ms | 84.2 ms | 88.8 ms |
| WASM (wasmtime 49) | 5 | 15.9 s | 57.7 ms | 77.2 ms | 81.8 ms |

All three targets of spec §29 are met: a world day in under 30 s (11–12 s), an epoch under 100 ms at the 95th percentile (54–59 ms), and WASM at most 3× slower than native (1.45×). The native and WASM runs end with the same state hashes.

The Merkle `state_root` that replaced the flat hash adds about 3.5 ms per epoch: seed 1 then takes 12.8 s per world day natively (p95 61.5 ms) and 18.3 s under WASM (p95 87.8 ms), still within every target, and both end at the same root. The reference core is not fixed yet (roadmap decision 6); a slower core has about 2.5× headroom on the world day.

## Findings (stage A3: maps and diversity)

Measured on 2026-09-26 over full 42-day seasons, 12 seeds per variant.

- **Every continent now gets founders and all five biomes.** Founder lineages go to the plates in turn and settle at least 4 cells from another plate; with the four per-plate biome mixes as the default, `maps --seeds 1..21` passes 20 of 20 seeds (9 before).
- **More niches, more clades.** With the per-plate mixes, at least 6 clades of 20+ hold for the whole season on 50% of seeds (10% before), and no clade holds 60% for more than 3 days on 100%. The dips are short and shallow: on the failing seeds the minimum is 5 (sometimes 3), against a mean of 8–10 clades, and they happen in every time of year while the continent breaks up, not only in winter.
- **Milder winters, earlier plague and more mutations do not help:** winter growth ×1.5 gives 16%, plague from a 20% share 41%, `mutation_ppm` 150,000 50%.
