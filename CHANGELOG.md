# Changelog

All notable changes to this project are documented in this file. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The project has no releases yet.

## [Unreleased]

### Added

- Stage A1 code:
  - `protogaea-core`, the deterministic simulation core;
  - `protogaea-harness`, the balance harness with HTML reports;
  - CI that checks formatting, lints and tests and compares state hashes across x86-64, ARM64, Windows, macOS and WASM.
- Specification v0.2 in English (canonical), with a Russian translation.
- Repository documentation: README in English and Russian, overview, FAQ, architecture, the spark protocol (draft), world rules (draft), glossary, roadmap, privacy, translations, decision records 0001–0013 and a publishing checklist.
- Policies: CONTRIBUTING, Code of Conduct (Contributor Covenant 2.1), SECURITY, LICENSING and TRADEMARKS (draft).
- The Apache License 2.0.

### Changed

- The working name changed from Pangea to **Protogaea**. Pangea is now the supercontinent of Season 1.
- Specification v0.2 replaces v0.1. The main changes: wishes with accumulated work replace the lottery; the Season 1 story "The Breaking of Pangea"; a changing environment and hunting with cyclic dominance; counter-based randomness; a spark log committed before the beacon; a bounded state with Merkle roots; open source from the first commit. The full list is in [spec §0](docs/spec/spec-v0.2.md#0-what-changed-since-v01).

## Specification 0.1 — 2026-09-24

- The original draft, in Russian: [docs/spec/archive/spec-v0.1.ru.md](docs/spec/archive/spec-v0.1.ru.md).

[Unreleased]: https://github.com/protogaea/protogaea/commits/main
