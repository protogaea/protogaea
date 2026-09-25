# Roadmap

**Where we are (2026-09-25):** specification v0.2 is complete, and stage A is in progress. Milestone A1 is done: a deterministic core and a balance harness. The world lives, but predators still die out by default — see the [harness findings](../harness/README.md#findings-stage-a1).

The riskiest question is not technical: *will people come back to watch?* The stages are ordered so that it is tested (stage B′) before the most expensive part, the spark infrastructure (stage C), is built.

## Stages

| Stage | Deliverables | Exit criteria | Status |
|---|---|---|---|
| A. Core and balance harness | Deterministic Rust core with a WASM build, cross-platform CI, the balance harness with metrics, the Season 1 map generator and rift schedule | The ecosystem health metrics ([spec §28](spec/spec-v0.2.md#28-ecosystem-health-the-balance-harness)) are met; roots match on all platforms; performance targets are reached | In progress: A1 done |
| B. Observation | Map, Muller plot, clade tree, cards, the WASM time machine, story detectors, museum, automatic names, "While you were away", subscriptions | In a usability test, 5–8 people explain the consequences of a mutation or an event without looking at server logs | Planned |
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
| 7 | Target values of the product metrics for B′ and D | Open — must be fixed before the tests |
| 8 | Legal review: wording, distribution of the spark client, app store rules, consent | Open |
| 9 | The name | Working name chosen: Protogaea. Legal check and registrations open |
| 10 | Licenses and contributions | Split decided; the data license and CLA vs DCO are open |
| 11 | English version of the specification | Done |

## Stage A in detail

**A2, part 1 — times of year and natural events (done, 2026-09-25):** a 372-epoch year with smooth transitions, moisture, wildfires, great droughts and plague; founders moved into the ruleset. Predators now survive 8 world days on 16 of 16 seeds.

**A1 — "Is the world alive?" (done, 2026-09-25):** items 1, 2 (without times of year, rifts, natural events, the museum and the spore bank), 4 (in CI) and 5 (the harness with the first checks) below. Next: the predator–prey balance (A1.1), then the rest of items 2–3 and 6–7 (A2), then tuning (A3).

1. **Repository skeleton.** A Rust workspace (core, replay command line, harness) and a CI matrix: x86-64 Linux and Windows, ARM64 Linux and macOS, WASM.
2. **Deterministic core.** State and tick order, counter-based randomness, genome and mutation, energy, hunting, movement choice, reproduction, death and decomposition, clades, museum and spore bank, times of year, natural events, rifts.
3. **Canonical serialization and the Merkle state root.**
4. **Determinism checks.** 500-epoch replays on several seeds, compared across all platforms on every commit.
5. **The balance harness.** A batch runner, the ecosystem health metrics, the archetype arena and a parameter search.
6. **Season 1 map generator.** Rift lines and schedule, and the published seed selection criteria.
7. **Performance.** Benchmarks against the targets: a world day in under 30 s on one reference core, an epoch in under 100 ms (p95), WASM at most 3× slower than native.

## Not planned

- **A token, NFTs or earnings** — never.
- **A spark client on phones** — not planned: app store rules and batteries.
- **A network of independent nodes** — decided in stage E, based on the results.
- **Sexual reproduction or evolving neural behavior** — possibly after stage E.
