# The specification

The specification is the single source of truth for Protogaea's design: the product, the world rules, the spark protocol, the architecture and the acceptance criteria.

| Version | File | Language | Status |
|---|---|---|---|
| 0.2 | [spec-v0.2.md](spec-v0.2.md) | English | **Canonical**, current |
| 0.2 | [translations/ru/spec-v0.2.ru.md](translations/ru/spec-v0.2.ru.md) | Russian | Translation |
| 0.1 | [archive/spec-v0.1.ru.md](archive/spec-v0.1.ru.md) | Russian | Archived — the original draft; only its working name was updated |

## Map of v0.2

| Part | Sections | Contents |
|---|---|---|
| — | §0 | What changed since v0.1 |
| I. Product and game design | §1–8 | Idea and player fantasy, world vocabulary, roles and the game loop, Season 1, actions, wishes and sparks, observation, the spark interface |
| II. World rules | §9–15 | Map and time, climate and natural events, organisms, clades and the museum, tick order, determinism, state and the log |
| III. The spark protocol | §16–21 | PoW, wishes, sparks and receipts, price and queue, epoch timeline, the trust boundary |
| IV. Architecture, platforms, operations, openness | §22–26 | Components, API, platforms and keys, operations, licenses and language |
| V. Acceptance and stages | §27–31 | Acceptance criteria, ecosystem health, checks, stages, decisions before code |
| Appendices | A–E | Glossary, decision log, risks, sources, comparable projects |

## How the specification changes

1. **Discuss first.** Open an issue with the "Spec feedback" template. Significant changes also get a [decision record](../decisions/README.md).
2. **English first.** Edit [spec-v0.2.md](spec-v0.2.md). The Russian translation is updated when a version is released, and its header says which version it follows.
3. **Versions.** A new version (0.3 and later) gets a new file and an updated §0 "What changed". Older versions move to `archive/`.
4. **Stable references.** Section numbers do not change within a version; other documents refer to sections as §N.
5. **Candidates and TBD.** Numbers marked "candidate" are expected to change after the balance harness exists. Open questions are marked TBD.
6. **Seasons are frozen.** Once a season starts, its rules come from its `ruleset` and do not change. A specification change that affects the rules applies from the next season.
