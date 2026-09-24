# 0006. One deterministic core, compiled to WASM

- **Status:** Accepted
- **Date:** 2026-09-24
- **Specification:** §14, §22

## Context

The same simulation must run in three places: on the server, in an independent command-line replay, and in the browser — for the time machine, counterfactuals, forecasts and root verification. Two implementations drift apart. JavaScript is a poor host for consensus arithmetic: its numbers are exact only up to 2⁵³. Common sources of non-determinism include floating point, hash-table iteration order, system time and overflow that behaves differently in debug and release builds.

## Decision

There is one core, written in Rust, compiled for the server, the replay and WASM. Inside the core: integers only, counter-based randomness ([0003](0003-counter-based-randomness.md)), no hash-table iteration, explicit saturating or checked arithmetic, and integer division only with a defined rounding rule. CI replays at least 500 epochs on several seeds and compares state roots on x86-64 Linux and Windows, ARM64 Linux and macOS, and WASM in Chrome, Firefox and Safari, on every commit.

## Consequences

- The browser can say "✓ verified on your device" and mean it.
- Performance targets become part of the contract: a world day in under 30 s on one reference core, and WASM at most 3× slower than native.
- A second, independent implementation of the core is still valuable for validating the specification; it is planned after the MVP.

## Alternatives considered

- **A separate JavaScript implementation for the browser** — doubles the maintenance and risks divergence.
- **Server-only computation** — gives up in-browser verification, which is at the heart of the trust model.
