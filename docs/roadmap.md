# Roadmap

**Where we are (2026-09-26):** specification v0.2 is complete, and stage A is in progress. Milestones A1 (a deterministic core and a balance harness) and A2 (times of year, natural events, the Breaking of Pangea, the museum and the spore bank) are done. A full 42-day season runs on every tested seed: the continent breaks apart as scheduled, and the world stays alive and diverse. Stage A3 (tuning) has started: the continents end the season with their own fauna — different lineages and different colors — while their traits converge, and the divergence check in spec §28 now measures the former ([harness findings](../harness/README.md#findings-stage-a3-why-the-continents-traits-do-not-diverge)). Every candidate seed now meets the map criteria (founders and all five biomes on each future continent). Diversity is now judged by its share of the season: at least 6 clades of 20+ during 95% of the time after day 3, so a brief dip during the breakup no longer fails a season. Performance meets its targets (a world day in about 13 s on one core), the state is committed by a Merkle `state_root` with organism proofs, and the archetype arena confirms the grazer–armored–hunter cycle on 19 of 20 seeds. Left in stage A: the parameter search, which is running; stage B (observation) has started with the world server.

The riskiest question is not technical: *will people come back to watch?* The stages are ordered so that it is tested (stage B′) before the most expensive part, the spark infrastructure (stage C), is built.

## Stages

| Stage | Deliverables | Exit criteria | Status |
|---|---|---|---|
| A. Core and balance harness | Deterministic Rust core with a WASM build, cross-platform CI, the balance harness with metrics, the Season 1 map generator and rift schedule | The ecosystem health metrics ([spec §28](spec/spec-v0.2.md#28-ecosystem-health-the-balance-harness)) are met; roots match on all platforms; performance targets are reached | Nearly done: A1 and A2 done, A3 tuning in its last step |
| B. Observation | Map, Muller plot, clade tree, cards, the WASM time machine, story detectors, museum, automatic names, "While you were away", subscriptions | In a usability test, 5–8 people explain the consequences of a mutation or an event without looking at server logs | Started: B1, the world server |
| B′. Closed observation test | 2–3 weeks, 30–100 invited participants; the full wish mechanic with a daily allowance of work units instead of PoW | The product metrics declared before the test are reached. If viewers do not come back, we improve the world, not the sparks | Planned |
| C. Sparks | yespower; the desktop app for Windows, Linux, macOS arm64 and Linux ARM64; the WASM spark client; the spark log with STHs and receipts; the ledger; the beacon; the watcher; load tests; signed builds | Sparks and miracle selection are verified independently; the engineering checks ([spec §29](spec/spec-v0.2.md#29-engineering-and-product-checks)) pass | Planned |
| D. Public Season 1 — The Breaking of Pangea | 42 world days; counterfactuals, chronicle, stream, hall of fame | No state divergence; real people create and discuss stories; product metrics | Planned |
| E. Decision on further development | An assessment of retention, costs and trust in the operator | Choose a direction: a network of independent nodes, private worlds for education, a deeper game, themes for the next seasons | Planned |

## Before writing code

These decisions come from [spec §31](spec/spec-v0.2.md#31-decisions-to-make-before-writing-code).

| # | Decision | Status |
|---|---|---|
| 1 | Final PoW algorithm and parameters, after benchmarks on x86-64, ARM64 and GPUs | Open |
| 2 | `ruleset` numbers from the balance harness; Season 1 seed selection criteria | Open — needs stage A |
| 3 | Beacon network, the "window close → round number" rule, the delay rule; independent watchers | Open |
| 4 | Formats of receipts, STHs and headers; log mirrors; OpenTimestamps cadence | Open — see the [protocol draft](protocol.md) |
| 5 | The root dictionary for names and the hypothesis templates | Open |
| 6 | The reference core and `P_min` in work units | Open |
| 7 | Target values of the product metrics for B′ and D | Proposed for B6, to be confirmed before it starts: of those who open the viewer, at least 40% come back the next day and 20% are still coming after a week; at least 60% follow a story or open a card; at least half make a prediction. Measured by the server's anonymous visit counts (`/v0/visits/summary`). Targets for B′ and D open |
| 8 | Legal review: wording, distribution of the spark client, app store rules, consent | Open |
| 9 | The name | Working name chosen: Protogaea. Legal check and registrations open |
| 10 | Licenses and contributions | Split decided; the data license and CLA vs DCO are open |
| 11 | English version of the specification | Done |

## Stage A in detail

**A3 — diversity over time, and the parameter search (2026-09-26):** spec §28 now asks for at least 6 clades of 20+ during 95% of the time after day 3 rather than at every moment, so a brief dip while the continent breaks up no longer fails a diverse season. `harness/search.py` runs full seasons for ruleset variants on the same seeds and ranks them by the §28 checks; the first screen varies twelve parameters one at a time.

**A3 — the archetype arena (2026-09-26):** part of item 5. The harness `arena` command pits the grazer, armored and hunter founders against each other in pairs; the intended cycle of spec §11.3 holds on 19 of 20 seeds ([findings](../harness/README.md#findings-stage-a3-the-archetype-arena)).

**A3 — the Merkle state root (2026-09-26):** item 3. The state is hashed as one Merkle tree per kind of data (cells, organisms, clades, museum, effects, rifts, spore bank, revivals, plus a leaf of global fields), shaped as in RFC 9162; an organism can be proven against `state_root` without the rest of the state. It costs about 3.5 ms per epoch, and native and WASM agree ([core README](../core/README.md#the-state-root)).

**A3 — performance (2026-09-26):** the harness `bench` command times each epoch on one thread. A world day takes 11–12 s natively and 16–17 s under WASI, an epoch 54–59 ms at the 95th percentile: all targets of item 7 are met with room to spare, and native and WASM end with the same state hashes ([findings](../harness/README.md#findings-stage-a3-performance)).

**A3, part 1 — divergence between continents (2026-09-25):** plate profiles in the harness; trait budget, per-plate biome mixes and a cold north as ruleset options. None of them makes the continents' traits diverge, but their clade makeup ends 86–100% apart and their hues 40–120° apart, so spec §28 now checks the fauna instead of the traits.

**A2, part 2 — the Breaking of Pangea (done, 2026-09-25):** plates, rift lines that flood in waves from the ocean inward, land bridges that close one by one, rescue or drowning on sinking cells, spring floods, the museum and the spore bank with natural revival; the `maps` command checks the Season 1 map criteria. CI compares hashes of a compressed season across platforms.

**Live preview (2026-09-25):** the harness can run a world in real time with snapshots and serve its report page behind a password: a first look at a living world before the stage B viewer.

**A2, part 1 — times of year and natural events (done, 2026-09-25):** a 372-epoch year with smooth transitions, moisture, wildfires, great droughts and plague; founders moved into the ruleset. Predators now survive 8 world days on 16 of 16 seeds.

**A1 — "Is the world alive?" (done, 2026-09-25):** items 1, 2 (without times of year, rifts, natural events, the museum and the spore bank), 4 (in CI) and 5 (the harness with the first checks) below. Next: the predator–prey balance (A1.1), then the rest of items 2–3 and 6–7 (A2), then tuning (A3).

1. **Repository skeleton.** A Rust workspace (core, replay command line, harness) and a CI matrix: x86-64 Linux and Windows, ARM64 Linux and macOS, WASM.
2. **Deterministic core.** State and tick order, counter-based randomness, genome and mutation, energy, hunting, movement choice, reproduction, death and decomposition, clades, museum and spore bank, times of year, natural events, rifts.
3. **Canonical serialization and the Merkle state root.**
4. **Determinism checks.** 500-epoch replays on several seeds, compared across all platforms on every commit.
5. **The balance harness.** A batch runner, the ecosystem health metrics, the archetype arena and a parameter search. Season 1 numbers are chosen for stories as well as for health: the harness counts what the story detectors of B4 find in every season (comebacks, invasions, crossings, changes of the dominant clade), and a world whose health checks pass but where nothing happens is not good enough.
6. **Season 1 map generator.** Rift lines and schedule, and the published seed selection criteria.
7. **Performance.** Benchmarks against the targets: a world day in under 30 s on one reference core, an epoch in under 100 ms (p95), WASM at most 3× slower than native.

## Stage B in detail

**B1 — the world server (2026-09-26):** `protogaea-server` runs the world on a timer, records every epoch's header, events, organisms and clades in SQLite, keeps snapshots, survives a crash without losing or changing an epoch, and serves the read API with inclusion proofs ([server README](../server/README.md)). It runs on the test server next to the stage A preview until the viewer (B2) replaces the preview.

**B5, part 1 — coming back (2026-09-26):** the viewer greets a returning viewer with "While you were away" (the population and the leading clade then and now, clades named and extinct, land bridges closed and the best stories, from `/v0/digest`), replays the last world day in about 30 seconds with the day's stories as captions (the server keeps compact frames of the last two world days at `/v0/replay`), and takes predictions for tomorrow on a clade's card ("will it still be alive this time tomorrow?", "will it be larger?"), checked the next day and scored in the browser. Next in B5: subscriptions to a clade or a region, with a Telegram bot.

**B4, part 1 — stories (2026-09-26):** the story detectors ([`stories/`](../stories/src/lib.rs)) find comebacks, crossings to another continent, invasions, arms races, the last organism of a great clade, a new world leader, clades split by a closing land bridge and the fall of great clades. The world server records them and serves the best of each day at `/v0/stories`, varied by clade and kind; the viewer shows them as "Today's stories" above the event feed. The harness counts them in every season (stories per world day, and by kind). On a compressed season the detectors find about six stories a world day. Next in B4: stories about continents by name, and choosing Season 1 numbers (and the world's size) for stories as well as health.

**B3 — names, the Muller plot and the clade tree (2026-09-26):** clades get an automatic binomial name (genus from the dominant trait, epithet from the preferred habitat) when they first reach 20 organisms; names are stored and never change. The viewer's Muller plot shows clade shares over the season with children inside their parents' bands and the season's phases and land bridge closings on the time axis; the clade tree shows the named clades on a time axis. The server folds the thousands of small clades into their nearest named ancestor.

**B2 — the viewer (2026-09-26, first version):** a TypeScript and PixiJS viewer served by the world server at `/app/`: the map with layers (biomes, food, organisms, rifts and their schedule, natural events), organisms drawn from their genomes when zoomed in, live mode in which organisms walk to their new cells each epoch, the season timeline with land bridge closings, clade and organism cards, the event feed and permanent links ([viewer README](../viewer/README.md)).

Stage B builds what a viewer sees, on top of the stage A core and without changing consensus. The server stays a single authoritative world (spec §22); sparks and wishes wait for stages B′ and C. Each milestone ends with something running on the test server.

1. **B1 — the world server and the event log.** A Rust server that runs the world on a timer and replaces the stage A live preview. Every epoch it records a header (epoch, `state_root` and its subtree roots, headline numbers), structured events (clades founded, named and extinct, land bridges closing, natural events, revivals) and, every few world hours, a snapshot. Indexes of clades and organisms, dead ones included, live in SQLite. It serves the read API of spec §23: world, ruleset, epochs, snapshots, clades, organisms, museum, events with stable cursors, and inclusion proofs. It resumes after a restart without losing an epoch.
2. **B2 — the viewer and the map.** A TypeScript viewer with a WebGL renderer (spec §22): the map with its layers (biomes, food, population, clade colors, rifts and their schedule, natural events), live mode that replays the latest epoch, a "jump to latest" button, and permanent links to every epoch, clade and organism. Organisms are drawn as procedural glyphs from the genome when zoomed in (spec §7).
3. **B3 — the Muller plot, the clade tree, cards and names.** The Muller plot for the whole season with natural events on the time axis; the phylogeny of clades; organism and clade cards; automatic binomial names from a screened dictionary of Latin roots.
4. **B4 — stories.** The story detectors of spec §7 (comeback, crossing, invasion, arms race, last of its kind, changing of the guard, records), plus the stories the Breaking of Pangea makes possible (a lineage crossing a land bridge before it closes, the last population of a clade on a continent). The feed shows one to three chosen stories a day, each with a named protagonist, what is at stake and when it will be decided; routine events stay available but out of the way. The same detectors run in the harness, so that every season can be scored for its stories as well as its health (stage A, item 5).
5. **B5 — coming back.** What makes a viewer return the next day: the "While you were away" digest, the last world day replayed in about 30 seconds with captions from the stories, and predictions for tomorrow (a simple form of the hypothesis journal of spec §7: "will this clade still be alive at this time tomorrow?", scored the next day, with no keys or signatures yet). Subscriptions to a clade or a region, delivered by a Telegram bot first.
6. **B6 — an early test with friends.** 10–20 invited people follow the world for a week with the digest in Telegram. The measures and their targets are fixed before it starts (decision 7 below): return on day 1 and day 7, the share who open a story or a card, predictions made. What they say shapes the rest of stage B and the stories of B4.
7. **B7 — the time machine and the archive.** The core built for the browser: open any past epoch from the nearest snapshot, recompute the ticks in WASM, compare two states, and check an organism's inclusion proof against the published `state_root`; the museum page and the daily chronicle from templates.
8. **B8 — the usability test.** 5–8 people explain the consequences of a mutation or an event without server logs (the stage B exit criterion); what they stumble on is fixed before stage B′.

Why this order: the riskiest question is whether people come back to watch. Stories, the digest and predictions are what answer it; the time machine matters for trust, which comes later with sparks. So they come first, and a small group of real viewers tests them before the rest of stage B is built.

## Not planned

- **A token, NFTs or earnings** — never.
- **A spark client on phones** — not planned: app store rules and batteries.
- **A network of independent nodes** — decided in stage E, based on the results.
- **Sexual reproduction or evolving neural behavior** — possibly after stage E.
