# 0008. Licensing split

- **Status:** Accepted (the data license is still to be decided)
- **Date:** 2026-09-24
- **Specification:** §26

## Context

Different parts of the project have different goals. Everything needed to verify the world should be as free as possible, so that anyone can build a watcher or a second implementation. The server and the web application are what someone would copy to run a competing hosted service, including private worlds for education, which may later fund the project. The world's history is valuable to researchers and archivists. The name must identify the official world.

## Decision

| Component | License |
|---|---|
| Simulation core, `ruleset`, protocol specification, replay, watcher, spark client | Apache-2.0 |
| Server, web application, desktop app (which includes the viewer) | AGPL-3.0 |
| World log, snapshots, season archives | CC0 or CC BY 4.0 — decided before Season 1 |
| Root dictionary, text templates, artwork | CC BY-SA 4.0 (candidate) |
| Name and logo | Not licensed; covered by the [trademark policy](../../TRADEMARKS.md) |

Today the repository contains only documentation, licensed under Apache-2.0.

## Consequences

- Verifiers get maximum freedom, including an explicit patent grant.
- Anyone who runs a public copy of the service must publish their changes.
- Every component directory needs its own `LICENSE` file.
- A CLA or DCO must be chosen before the first external contribution; without a CLA, the license of accepted code cannot be changed later.

## Alternatives considered

- **One license for everything:** Apache-2.0 would let a closed hosted clone appear; AGPL-3.0 would deter independent verifiers and embedders.
- **MIT instead of Apache-2.0** — no explicit patent grant.
