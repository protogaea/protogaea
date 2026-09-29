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

# The archetype arena: grazers, armored organisms and hunters in pairs, mutation off
cargo run --release -p protogaea-harness -- arena --seeds 1..21 --days 3
# One hunters-versus-grazers world, day by day
cargo run --release -p protogaea-harness -- arena --trace 5

# Patron bots (spec v0.3, draft): each strategy over the seeds, the §28 checks and the patrons' measures
cargo run --release -p protogaea-harness -- patrons --seeds 1..17 --days 14 --work 150

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
- at least 6 clades of 20+ organisms during 95% of the time after day 3;
- the dominant clade changes at least once per 3 days;
- no clade holds more than 60% of the population for more than 3 days;
- under 1% of ticks at the global limit;
- equilibrium population at 40–70% of the limit;
- at least 30 generations per world day;
- each plate ends as its own continent (judged once the last land bridge has closed);
- each continent ends with its own fauna: the clade makeup of different continents is at least 80% apart and their mean hues at least 30° apart at the end of the season (judged on runs of a full season).

## Stories

Every run and sweep also runs the story detectors of [`protogaea-stories`](../stories/src/lib.rs) and reports how many stories a season tells per world day and of which kinds (`st/day` in the sweep table; `stories_per_day` and `story_*` columns in the CSV; the best stories at the end of `run`). A season whose health checks pass but where nothing happens is not what Season 1 needs, so the parameter search ranks by stories too.

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

## Findings (stage A3: the archetype arena)

Measured on 2026-09-26 with `arena --seeds 1..21 --days 3`: the grazer, armored and hunter founders of the default ruleset, 100 of each, in pairs, with mutation and the spore bank off and no rifts, the second lineage starting next to the first; each plant eater also runs alone. The verdicts follow spec §11.3: grazers beat armored organisms if they hold the larger share at the end, armored organisms beat hunters if the hunters starve out, hunters beat grazers if they survive and keep the grazers at 60% or less of their numbers alone.

- **Grazers beat armored organisms on 20 of 20 seeds**, with about 60% of the population against 40%.
- **Armored organisms beat hunters on 20 of 20**: the hunters starve within the first day.
- **Hunters beat grazers on 19 of 20**, holding them at 10–55% of the 3,650 they reach alone. On seed 14 the hunters ate every grazer and then starved.
- **The full cycle holds on 19 of 20 seeds.**

Genesis puts each founder in its own biome, and at first the lineages often started far apart: on 3 of 9 seeds the hunters starved before they found any grazer. The arena now moves the second lineage next to the first, so that it measures the archetypes against each other rather than the distance between them.

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

**2026-09-29: 3.2–3.6 times faster.** By then the rules had grown and a world day took 14.0 s (Season 1 rules) and 14.5 s (v0.3 draft) on one thread of the same CPU. A profile showed where it went: about 42% in the BLAKE3 hash behind every random draw, 14% in genome distances at the kin check, and much of the rest in movement weighing the strongest hunter around every cell in sight, rescanning its 3 × 3 area for every organism. Two steps:

| Step | Season 1 world day | v0.3 world day | State roots |
|---|---|---|---|
| Before | 14.0 s | 14.5 s | — |
| The hunters around each cell found once a tick, the attack margin checked before kinship, the genome distance on flat lanes | 7.9 s | 9.3 s | unchanged, bit for bit |
| Philox4x32-10 instead of one BLAKE3 hash per draw ([decision 0014](../docs/decisions/0014-philox-for-the-draws.md)) | 3.9 s | 4.6 s | every world changes |

An epoch now takes 13.8 ms at the median and 18.5 ms at the 95th percentile (Season 1 rules). One draw costs 11.6 ns natively and 16.1 ns under wasmtime instead of 134 and 177 ns. With the new generator all 20 candidate seeds still meet the Season 1 map criteria, and 46 seasons of 42 days (seeds 1–11 and 13–47) pass the §28 checks as with the old generator: 10.6 of 11 checks per season against 10.7, the same mean continent composition (915–952 per mille against 923–958) and hue divergence (80–91° against 80–96°). One number is lower on both seed sets: the leader changes 2.4 times per 3 days on average against 2.7 (1.9 against 2.5 on seeds 1–11, 2.5 against 2.8 on 13–47), still above the check's one. The 35 seasons of seeds 13–47 took 2193 s on 12 threads with the old generator; the first eleven took 280 s instead of 800 s. Building with `target-cpu=native` or fat LTO gained nothing worth keeping (LTO about 2%).

## Findings (stage A3: maps and diversity)

Measured on 2026-09-26 over full 42-day seasons, 12 seeds per variant.

- **Every continent now gets founders and all five biomes.** Founder lineages go to the plates in turn and settle at least 4 cells from another plate; with the four per-plate biome mixes as the default, `maps --seeds 1..21` passes 20 of 20 seeds (9 before).
- **More niches, more clades.** With the per-plate mixes, at least 6 clades of 20+ hold for the whole season on 50% of seeds (10% before), and no clade holds 60% for more than 3 days on 100%. The dips are short and shallow: on the failing seeds the minimum is 5 (sometimes 3), against a mean of 8–10 clades, and they happen in every time of year while the continent breaks up, not only in winter.
- **Milder winters, earlier plague and more mutations do not help:** winter growth ×1.5 gives 16%, plague from a 20% share 41%, `mutation_ppm` 150,000 50%.

## Findings (stage A3: the size of the world)

Measured on 2026-09-26 over full 42-day seasons on fresh seeds 101 to 113 (12 seeds each), default rules; at 128 × 128 the population cap and the founders were scaled with the area (×4). Twelve threads of an AMD Ryzen 5 5500 on the test server.

| Size | Seeds alive | Checks passed (mean) | 6+ clades of 20+ 95% of the time | Continents apart at the end | Stories per world day | Time for the 12 seasons |
|---|---|---|---|---|---|---|
| 64 × 64 | 12 of 12 | 9.8 | 12 of 12 | 58% | 4.5 (2.7–6.6) | 11 min |
| 128 × 128 | 12 of 12 | 9.6 | 12 of 12 | 58% | 6.4 (4.5–8.8) | 50 min |

- **Health is the same at both sizes.** The larger world does not make continents diverge more.
- **More stories, but of the routine kinds.** A season at 128 × 128 tells about twice as many crossings (167 against 83) and three times as many comebacks (50 against 15), but fewer of the rarer ones: new leaders 4 against 17, invasions 28 against 45, the last of a great clade 6 against 19. A larger world has a steadier leader.
- **It costs 4.4 times as much:** about 72 s of one core per world day, against the target of 30 s (spec §29), and four times the snapshots, replay frames and time-machine work in the browser.
- **Arms races were almost never told** at either size (0.1 and 0 a season): the detector compared world-wide daily means, which swing with the hunters' booms and busts (15 to 500 hunters from one day to the next). It now compares two three-day windows with enough hunters in both; with the Season 1 numbers it tells 7 arms races in 12 seasons, in half of them.

Season 1 stays at 64 × 64 unless the core gets about 2.5 times faster.

## Findings (spec v0.3 draft: patrons of clades)

The [v0.3 draft](../docs/spec/spec-v0.3-draft.md) lets players back clades: easing for their own clade (`shelter`, `forage`, `cure`), harm to a rival's (`blight`, `expose`, `sickness`), six heritable gifts, and weather as a natural event. It lives in the core behind the ruleset's optional `patrons` section: a ruleset without it keeps its id, and worlds without it keep their roots (checked byte for byte against `hash`). `patrons` runs bots that put 150 work units an epoch into wishes (about 1.5 miracles an epoch at a 10% share) under eight strategies: none, for the leader, for the weakest, random, a whale with half of all work behind one lineage, harassing the smallest clade, a war of two camps, and a mix.

Four runs of 14 world days, the last on 16 seeds (2026-09-27):

- **The world holds under every strategy.** No extinction; 6+ clades of 20+ during 99–100% of the time after day 3; no clade above 60% for more than a day.
- **Helping the leader had to cost more.** With a price ×2.5 at a 20% share, the leader changed less than once in 3 days (0.96). Priced at `share² / 100` (×4 at 20%, ×9 at 30%, none at 50%+) it changes 1.6 ± 0.3 times, the no-player baseline being 1.9 ± 0.6.
- **Harassment had to be bounded.** At 2% protection and a 12-epoch respite, 12% more named clades died than without players; at 3% and 24 epochs, 348 ± 11 against 350 ± 15.
- **A lineage's long lead is natural.** The lineage leading after day one holds over 60% of the living for about 9 days and ends ahead of the second on 15 of 16 seeds with no players at all; a whale (9.1 days) and a war (13 of 16) do not change it.
- **Gifts:** with five gifted organisms and 2% loss per birth, most gifts were lost to drift within 3 days and `scavenge` kept 91%; with ten gifted, 0.5% loss, `scavenge` at a third of food's value and a heavier upkeep, 20–34% of gifts are still held 3 days later, `keen` 16% (now +4 defense).

## Findings (spec v0.3 draft: size, longevity and new niches)

Stage 1b adds two traits behind the ruleset's optional `traits8` section (a budget of 32 over eight traits): **size** (more energy held, attack and defense, a higher threshold to breed and a stronger newborn, for more upkeep) and **longevity** (aging and death later, for more upkeep); and three niches: scavengers by birth (plant eating 6+ and hunting 3+ eat detritus), coastal swimmers (algae grow in the shallows, eaten only with `swim`), and burrowers (movement 3 or less, defense 5+, hide from a hunter next to them). `Ruleset::v03()` gathers it with the patrons and ten founders of 40 (the six of v0.2 and a scavenger, a swimmer, a burrower and a giant); `ruleset v03` prints it and `patrons --v03 yes` runs it. Worlds without the section keep their roots (checked with `hash`).

- **The price of the traits sets the world.** Cheap longevity (4 a point, +10% of life) drove every lineage to longevity 7–8; dear (12, +5%) to 0; cheap size (4, +12% energy) let giants take the world (500–1350 of them). A grid of nine prices (no players, 8 seeds × 7 days) found longevity 8 a point with +7% of life and size 6 with +11% energy: both stay in the middle, and all four niches live.
- **On 16 seeds × 14 days with the eight strategies** (2026-09-28): no extinction; 6+ clades of 20+ during 90–98% of the time after day 3 (98% without players); no clade above 60% for more than 1.2 days; mean size 1.7–2.5 and longevity 3.3–5.5; swimmers 150–450 and giants 24–68 at the end, burrowers 4–7 and scavengers 2–33 (small niches); 352–432 named clades extinct (about 350 in v0.2). A clade splits at 4 steps instead of 3, or eight traits make it split too often (545 extinct).

## Findings (spec v0.3 draft: hybridization)

Stage 1c adds `hybrid` behind the `patrons` section: two living clades with 5+ members each in one 5 × 5 area and reference genomes 2–8 steps apart found a new clade with two parents. Its members take each gene from one parent by counter-based randomness, the trait budget is then evened out one point at a time, and each gift that at least half of a parent's members around the center hold passes with 50%, at most two. A pair is crossed at most once a world day, whichever way round. The clade's second parent enters `state_root` only for hybrids, so other worlds keep their roots. The helping bots cross their clade with the largest compatible neighbour as one of five kinds of help (price ×2.5), and the report follows each hybrid clade, with the clades descended from it, a day and three days on. To compare with, it follows clades founded by mutation: a sample of all of them, and those under an hour old that already have as many members as a hybrid clade starts with.

- **A hybrid clade lives about as long as a young clade of its size.** With 4 hybrids (2026-09-29, 16 seeds × 14 days, eight strategies), 5% of 3715 hybrid clades were alive with their descendants a day later and 2% three days later; on 3 strategies × 16 seeds × 7 days, young mutation clades of 4+ members lived 7% and 3%. With 8 hybrids, 9% and 3% against 10% and 4% for young clades of 8+. Survival is set by the size of the start, not by the crossing; one clade founded by mutation in a hundred lives a day. 8 is now the default (at most 9 fit in 3 × 3).
- **The world holds with them.** With 8 hybrids on 16 seeds × 14 days: no extinction; 6+ clades of 20+ during 95–99% of the time after day 3; no clade above 60% for more than 0.9 days; 332–425 named clades extinct; the §28 checks pass in 70–78% of seasons (77–80% in the run before hybridization, the weak strategy lowest). 4078 hybrid clades were founded, about 36 a season under a helping strategy.

## Findings (stage A3: the Season 1 numbers)

The parameter screen (`search.py screen`, 24 one-parameter changes on seeds 1 to 13) was noisy at 12 seeds, so its five best changes were combined and checked on fresh seeds 201 to 213 that the screen never saw (2026-09-26):

| Variant | Checks passed (mean of 11) | 6+ clades of 20+ 95% of the time | Continents apart at the end | Stories per world day |
|---|---|---|---|---|
| defaults before | 9.83 | 75% | 75% | 4.2 |
| hunting at 70% energy + plague mortality 65% | 10.25 | 100% | 92% | 5.7 |
| all five changes | 10.75 | 92% | 83% | 6.9 |
| all but the plague change | **10.75** | **100%** | **92%** | **8.0** |

The last one is now the default: `hunt_hunger_pct` 60 → 70, `kin_distance` 2 → 1, `mutation_ppm` 100,000 → 140,000, `weights.habitat_bonus` 200 → 300. Every check passes on 91–100% of seeds (a changing leader every 3 days went from 58% to 91%), and every kind of story is told more often: new leaders 50 a season against 23, invasions 59 against 34, the last of a great clade 37 against 22. A world day still takes 11.6 s (p95 epoch 57 ms).

