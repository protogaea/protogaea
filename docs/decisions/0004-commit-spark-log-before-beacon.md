# 0004. Commit the spark log before the beacon round

- **Status:** Accepted
- **Date:** 2026-09-24
- **Specification:** §18, §20, §21

## Context

The beacon value seeds each epoch and breaks ties in the ledger, so it must not be predictable when sparks are submitted. Receipts let naturalists prove that the operator dropped their sparks, but receipts alone do not stop the operator from *adding* sparks. If the final set of sparks were fixed after the beacon value became known, the operator could hold back honestly produced sparks and then include only those that lead to a favorable outcome — grinding by selective inclusion.

## Decision

The spark log of each epoch is a Merkle tree following Certificate Transparency (RFC 6962). Signed tree heads (STHs) are published at least every 2 seconds, and every receipt carries an STH and an inclusion proof. The **final STH is published no later than 2 seconds after the window closes**, and the beacon round used for the epoch is the first round at least 10 seconds after the close. Independent watchers record when they first saw each STH. An STH first seen after the round time is a public violation.

## Consequences

- The operator cannot steer outcomes by choosing which sparks to include after seeing the beacon value.
- Anyone holding a receipt can prove that their spark was dropped (consistency proofs).
- The design depends on watchers existing and recording times, so at least two independent watchers are required before the public season.
- Each epoch waits for the beacon, adding about 10–13 seconds between windows.

## Alternatives considered

- **Publishing the list of sparks after the close without a commitment** — leaves the grinding attack open.
- **Commit–reveal by the participants** — more complex for users, and it does not remove the operator's advantage.
