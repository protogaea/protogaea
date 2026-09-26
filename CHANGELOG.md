# Changelog

All notable changes to this project are documented in this file. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project has no releases yet.

## [Unreleased]

### Added

- Anonymous visit counts for the friends test (`POST /v0/visits`, `/v0/visits/summary`): returns on day 1 and day 7 and what visitors did, under a random id per browser, kept apart from the world's data.
- Stage B5, part 1: "While you were away" (`/v0/digest`), the last world day replayed in about 30 seconds with story captions (`/v0/replay`, compact frames of the last two world days) and predictions for tomorrow in the viewer.
- Stage B4, part 1: story detectors (`protogaea-stories`) in the harness, the world server (`/v0/stories`, kept across restarts) and the viewer ("Today's stories").
- Stage B3: automatic binomial clade names, stored once a clade reaches the naming threshold; the Muller plot and the clade tree in the viewer; `/v0/tree` and a Muller plot folded to named clades on the server.
- Stage B2, the viewer (`viewer/`): the map with layers and genome glyphs, live mode, the season timeline, clade and organism cards, the event feed and permanent links, in English and Russian; the server serves it at `/app/` and its API gains the traits on the map, past epochs from snapshots and a newest-first feed.
- Stage B1, the world server (`protogaea-server`): runs the world on a timer, records every epoch's header, events, organisms (the dead included), clades and Muller samples in SQLite, keeps snapshots, recovers from a crash without losing or changing an epoch, and serves the read API of spec §23 with inclusion proofs; a systemd unit in `deploy/`.
- The world run with its stand-in beacon moved from the harness into the core (`protogaea_core::run`), and the epoch report lists every death with its cause.
- Harness: `diverse_pct_after_day_3`, the share of the time after day 3 with at least 6 clades of 20+, and `harness/search.py`, a parameter search over full seasons.
- Stage A1 code:
  - `protogaea-core`, the deterministic simulation core;
  - `protogaea-harness`, the balance harness with HTML reports;
  - CI that checks formatting, lints and tests and compares state hashes across x86-64, ARM64, Windows, macOS and WASM.
- Stage A2, part 1: times of year and moisture, wildfires with ash, great droughts, plague ("kill the winner"); founders in the ruleset.
- Stage A2, part 2: the Breaking of Pangea (plates, rift lines, flooding waves, land bridges, organisms carried off sinking cells or drowned), spring floods, the museum of extinct named clades, the spore bank with natural revival and the end of a season by extinction.
- Harness: the `maps` command for the Season 1 map criteria, continents and divergence between continents in the metrics, and a map that follows the breakup.
- CI compares state hashes of a compressed season, so that the rift code is checked across platforms too.
- Harness: the `arena` command (spec §28) pits the three archetypes against each other in pairs; the grazer–armored–hunter cycle holds on 19 of 20 seeds.
- The Merkle `state_root` of spec §15: one tree per kind of data plus a leaf of global fields, in the shape of RFC 9162 with BLAKE3, and inclusion proofs for organisms (`World::prove_organism`, `OrganismProof::verify`). It replaces the flat state hash everywhere, including the epoch seed; the encoding tag is `PROTOGAEA/STATE/A3`.
- Harness: the `bench` command times epochs on one thread against the performance targets of spec §29; natively a world day takes 11–12 s, under WASI 16–17 s.
- Season 1 maps: founder lineages are spread over the plates, away from the future rifts, and the plates get four different biome mixes by default, so every candidate seed gives each future continent founders and all five biomes; this also doubles the seasons that keep 6+ clades of 20+.
- Stage A3 tools: the harness prints each plate's population, dominant clade and mean traits during `run`, and measures the distance between the plates' mean genomes (`cdiv+`) next to the divergence of dominant clades. The ruleset gains `trait_budget` (24), `biome_mix`, optional per-plate `plate_mixes` and an optional cold north (`cold_winter_pct`); the defaults leave worlds unchanged. The harness also measures how different the continents' clade makeup (`comp%→`) and mean hues (`hue°`) are.
- Hunting balance: satiation and cover (spec §11.3).
- Live mode in the harness: a world in real time with snapshots, its page behind an optional password, and a systemd unit in `deploy/`.
- A static Linux build (`x86_64-unknown-linux-musl`) that needs no C toolchain: BLAKE3 uses its portable implementation.
- Specification v0.2 in English (canonical), with a Russian translation.
- Repository documentation: README in English and Russian, overview, FAQ, architecture, the spark protocol (draft), world rules (draft), glossary, roadmap, privacy, translations, decision records 0001–0013 and a publishing checklist.
- Policies: CONTRIBUTING, Code of Conduct (Contributor Covenant 2.1), SECURITY, LICENSING and TRADEMARKS (draft).
- The Apache License 2.0.

### Changed

- The world server and the viewer are licensed under AGPL-3.0-only, as planned in LICENSING.md and spec §26; the core, the harness and the documentation stay under Apache-2.0.
- Specification §28: at least 6 clades of 20+ must hold during 95% of the time after day 3 instead of at every moment, so a brief dip while the continent breaks up does not fail a diverse season.
- Specification §28: divergence after the breakup is judged by the continents' fauna — clade makeup at least 80% apart and mean hues at least 30° apart at the end of the season — instead of the growth of the distance between dominant clades' traits, which converge under the same rules.

- Specification §10 describes the rift generator, rescue and drowning, and floods in detail; §12 says revived genomes found new clades; §15 lists the rift schedule and the revivals in the state.
- Specification §10 now matches the core: moisture drifts toward a seasonal target (`moisture_base`, `season_moisture_delta`); values between times of year are interpolated linearly from their midpoints; a great drought also drops moisture by 30.
- The working name changed from Pangea to **Protogaea**. Pangea is now the supercontinent of Season 1.
- Specification v0.2 replaces v0.1. The main changes: wishes with accumulated work replace the lottery; the Season 1 story "The Breaking of Pangea"; a changing environment and hunting with cyclic dominance; counter-based randomness; a spark log committed before the beacon; a bounded state with Merkle roots; open source from the first commit. The full list is in [spec §0](docs/spec/spec-v0.2.md#0-what-changed-since-v01).

## Specification 0.1 — 2026-09-24

- The original draft, in Russian: [docs/spec/archive/spec-v0.1.ru.md](docs/spec/archive/spec-v0.1.ru.md).

[Unreleased]: https://github.com/protogaea/protogaea/commits/main
