# 0012. `revive` instead of `found_lineage`

- **Status:** Accepted
- **Date:** 2026-09-24
- **Specification:** §5, §12

## Context

Specification v0.1 let players found a lineage with an arbitrary genome of 24 points. The rules are public and anyone can run the simulation locally, so players would quickly compute optimal builds and fill the world with them, crowding out everything evolution produced. The museum of extinct clades, meanwhile, was only a display.

## Decision

New lineages enter the world by **revival**:

- The source is either a museum entry (a clade extinct for at least 36 epochs) or the spore bank (the genesis genomes and, from Season 2, the survivors of the previous season).
- The genome may be edited by at most two mutation steps.
- A revival places 5 organisms in a 3 × 3 area, and the new clade becomes a child of the museum entry.
- Each entry can be revived at most once per world day. The author picks a name from the dictionary of Latin roots.

## Consequences

- Every genome in the world has a natural origin.
- The museum gains a role in play, and revivals create stories: "the ancients have returned".
- `revive` is available from the first day, thanks to the spore bank.
- Players have less creative freedom. We accept that in exchange for a world that belongs to evolution.
- The state keeps a bounded museum (the last 1,024 entries) so that the core can validate revivals without external data.

## Alternatives considered

- **Free design** — turns into min-maxing.
- **Free design only at the start of a season** — the same problem, concentrated at the start.
- **No new lineages at all** — loses a strong source of stories.
