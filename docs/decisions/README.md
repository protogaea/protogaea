# Decision records

We record significant decisions as short Architecture Decision Records (ADRs): the context, the decision, its consequences and the alternatives we rejected. The [specification](../spec/spec-v0.2.md) says *what* the system does; these records say *why*. Its [decision log (Appendix B)](../spec/spec-v0.2.md#b-decision-log) is the one-line summary of the same decisions.

| # | Decision | Status | Date |
|---|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record decisions as ADRs | Accepted | 2026-09-25 |
| [0002](0002-wishes-instead-of-lottery.md) | Wishes with accumulated work instead of a lottery | Accepted | 2026-09-24 |
| [0003](0003-counter-based-randomness.md) | Counter-based randomness | Accepted | 2026-09-24 |
| [0004](0004-commit-spark-log-before-beacon.md) | Commit the spark log before the beacon round | Accepted | 2026-09-24 |
| [0005](0005-single-authoritative-server-in-v0.md) | A single authoritative server in v0 | Accepted | 2026-09-24 |
| [0006](0006-one-deterministic-core-compiled-to-wasm.md) | One deterministic core, compiled to WASM | Accepted | 2026-09-24 |
| [0007](0007-open-source-from-first-commit.md) | Open source from the first commit | Accepted | 2026-09-24 |
| [0008](0008-licensing-split.md) | Licensing split | Accepted | 2026-09-24 |
| [0009](0009-name-protogaea.md) | The name: Protogaea | Accepted | 2026-09-25 |
| [0010](0010-english-first.md) | English first | Accepted | 2026-09-25 |
| [0011](0011-world-vocabulary.md) | World vocabulary: sparks, wishes, miracles | Accepted | 2026-09-24 |
| [0012](0012-revive-instead-of-found-lineage.md) | `revive` instead of `found_lineage` | Accepted | 2026-09-24 |
| [0013](0013-yespower-as-candidate-pow.md) | yespower as the candidate proof of work | Proposed | 2026-09-24 |

## Adding a record

1. Copy [template.md](template.md) to the next number: `NNNN-short-title.md`.
2. Start with the status **Proposed** and discuss it in an issue.
3. When the decision is made, set the status to **Accepted** or **Rejected** and update the specification in the same change.
4. Never rewrite an accepted record. To change a decision, write a new record and mark the old one **Superseded by NNNN**.
5. A decision that changes the rules of a running season takes effect only in the next season.
