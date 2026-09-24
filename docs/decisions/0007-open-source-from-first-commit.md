# 0007. Open source from the first commit

- **Status:** Accepted
- **Date:** 2026-09-24
- **Specification:** §26

## Context

The trust model relies on anyone being able to recompute the world. Replay, watchers and in-browser verification mean nothing if the core and the client are closed: a closed client could display "✓ verified" without verifying anything. The code would have to be opened by stage C anyway. Meanwhile, a program that loads the CPU with proof of work is exactly what antivirus products and cautious users distrust.

## Decision

The code is open from the first commit. The project is announced loudly only after stage B, when there is something to show. Openness of the code and access to the world are separate: the world stays closed (invitation-only) through stage B′. What stays private: the operator's keys and secrets, abuse-protection thresholds and infrastructure configuration.

## Consequences

- A commit history from day one shows that the rules were never changed quietly.
- An open reference spark client levels the field: every optimization reaches everyone.
- Open code and reproducible builds are the main defense against antivirus alerts and suspicions of hidden mining.
- Forks — including forks with a token — become possible. The trademark policy ([0008](0008-licensing-split.md)) and the living community are the defense.
- External pull requests wait until a CLA or DCO is chosen.

## Alternatives considered

- **Closed until launch** — breaks the trust model and only delays forks.
- **Source-available** — does not allow independent implementations and watchers without permission.
