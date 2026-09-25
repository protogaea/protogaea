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
- the mean genome distance between the dominant clades of different continents grows by at least 3 steps from the start of phase III to the end of the season (judged on runs of a full season).

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
