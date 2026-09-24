# 0001. Record decisions as ADRs

- **Status:** Accepted
- **Date:** 2026-09-25
- **Specification:** Appendix B

## Context

Protogaea is open from the first commit, and many of its decisions are unusual: proof of work without a token, a single server that anyone can verify, rules frozen for a season. Newcomers will ask "why not X?" for years. The specification says what the system does; it is too long to also carry the full reasoning behind every choice.

## Decision

Significant decisions are recorded as short Architecture Decision Records in `docs/decisions/`. Each record has a context, a decision, consequences and rejected alternatives. The specification's decision log (Appendix B) keeps a one-line summary and links here.

## Consequences

- Reasons survive after the conversation that produced them is forgotten.
- Changing a decision means writing a new record, which keeps the history honest.
- There is a small cost: every significant change needs a record.

## Alternatives considered

- **Only the decision log in the specification** — too short to hold the reasoning.
- **Discussions in issues only** — the reasoning gets scattered and hard to find.
