# 0005. A single authoritative server in v0

- **Status:** Accepted
- **Date:** 2026-09-24
- **Specification:** §21, §22

## Context

Proof of work invites the idea of a decentralized network. A real network, however, would need every node to verify PoW, event order, randomness and full recomputation, and to resolve competing branches. That is a separate protocol with its own security analysis. It could take longer to build than the rest of the project, and it would not answer the most important question: will people come back to watch?

## Decision

v0 runs one authoritative server. Everything it publishes is verifiable: the deterministic core, the public log, Merkle state roots, the spark log committed before the beacon, OpenTimestamps anchoring and independent watchers. The remaining trust boundary is stated openly: the operator can refuse sparks, delay publication or stop the world. A network of independent nodes is decided in stage E, based on the results.

## Consequences

- Much simpler to build and to run: one server with 4–8 cores, object storage and a CDN.
- Dishonesty that matters is detectable; censorship and downtime are visible but not preventable.
- The project must never describe itself as decentralized in v0.
- The world handover plan (§26) lowers the risk that the world dies with its operator.

## Alternatives considered

- **A peer-to-peer network from the start** — too costly before the core hypothesis is tested.
- **A public blockchain** — contradicts "no token", adds fees and speculation, and was rejected by design.
