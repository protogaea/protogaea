# Changelog

All notable changes to this project are documented in this file. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project has no releases yet.

## [Unreleased]

### Added

- Stage A1 code:
  - `protogaea-core`, the deterministic simulation core;
  - `protogaea-harness`, the balance harness with HTML reports;
  - CI that checks formatting, lints and tests and compares state hashes across x86-64, ARM64, Windows, macOS and WASM.
- Stage A2, part 1: times of year and moisture, wildfires with ash, great droughts, plague ("kill the winner"); founders in the ruleset.
- Stage A2, part 2: the Breaking of Pangea (plates, rift lines, flooding waves, land bridges, organisms carried off sinking cells or drowned), spring floods, the museum of extinct named clades, the spore bank with natural revival and the end of a season by extinction.
- Harness: the `maps` command for the Season 1 map criteria, continents and divergence between continents in the metrics, and a map that follows the breakup.
- CI compares state hashes of a compressed season, so that the rift code is checked across platforms too.
- Stage A3 tools: the harness prints each plate's population, dominant clade and mean traits during `run`, and measures the distance between the plates' mean genomes (`cdiv+`) next to the divergence of dominant clades. The ruleset gains `trait_budget` (24), `biome_mix` and optional per-plate `plate_mixes`; the defaults leave worlds unchanged.
- Hunting balance: satiation and cover (spec §11.3).
- Live mode in the harness: a world in real time with snapshots, its page behind an optional password, and a systemd unit in `deploy/`.
- A static Linux build (`x86_64-unknown-linux-musl`) that needs no C toolchain: BLAKE3 uses its portable implementation.
- Specification v0.2 in English (canonical), with a Russian translation.
- Repository documentation: README in English and Russian, overview, FAQ, architecture, the spark protocol (draft), world rules (draft), glossary, roadmap, privacy, translations, decision records 0001–0013 and a publishing checklist.
- Policies: CONTRIBUTING, Code of Conduct (Contributor Covenant 2.1), SECURITY, LICENSING and TRADEMARKS (draft).
- The Apache License 2.0.

### Changed

- Specification §10 describes the rift generator, rescue and drowning, and floods in detail; §12 says revived genomes found new clades; §15 lists the rift schedule and the revivals in the state.
- Specification §10 now matches the core: moisture drifts toward a seasonal target (`moisture_base`, `season_moisture_delta`); values between times of year are interpolated linearly from their midpoints; a great drought also drops moisture by 30.
- The working name changed from Pangea to **Protogaea**. Pangea is now the supercontinent of Season 1.
- Specification v0.2 replaces v0.1. The main changes: wishes with accumulated work replace the lottery; the Season 1 story "The Breaking of Pangea"; a changing environment and hunting with cyclic dominance; counter-based randomness; a spark log committed before the beacon; a bounded state with Merkle roots; open source from the first commit. The full list is in [spec §0](docs/spec/spec-v0.2.md#0-what-changed-since-v01).

## Specification 0.1 — 2026-09-24

- The original draft, in Russian: [docs/spec/archive/spec-v0.1.ru.md](docs/spec/archive/spec-v0.1.ru.md).

[Unreleased]: https://github.com/protogaea/protogaea/commits/main
