# 0002. Wishes with accumulated work instead of a lottery

- **Status:** Accepted
- **Date:** 2026-09-24
- **Specification:** §6, §19

## Context

Specification v0.1 allocated interventions by lottery: every valid ticket was one chance at one of three slots per epoch. That scales badly in both directions:

- with 1,000 equal participants, the chance of winning during a 15-minute session is about 0.9%, and a win takes about 28 hours of mining on average — it feels like a slot machine that burns electricity;
- with 10 participants, each one wins roughly every 17 minutes, so interventions are nearly free and the world belongs to the players rather than to its ecology.

The lottery also left open what happens when two tickets of the same proposal win, and it gave nobody a reason to cooperate.

## Decision

Interventions are public **wishes**. Anyone can kindle sparks for any wish, and the work accumulates across epochs. When it reaches the **price**, the wish is queued; at most three miracles execute per epoch. The price behaves like difficulty — up when more wishes are ready than there are slots, down otherwise — with a floor, so that a miracle always costs noticeable work, even with five players. Bigger actions cost more (`price_mult`).

## Consequences

- Progress is visible and no work is wasted until a wish expires; this makes cooperation and rivalry possible — "save the blue branch", "rain versus drought".
- Arrival order within a window no longer matters, so there is no race to protect against.
- Proportional influence stays: whoever has half the computing power still has about half the influence. We say so openly and publish concentration statistics.
- The ledger becomes part of consensus (`ledger_root`).

## Alternatives considered

- **Keep the lottery with fixes** (drawing by proposal, a minimum work per slot) — still frustrating at scale and still solitary.
- **Best-hash per epoch** — simpler verification, but the same variance problem.
- **Per-key caps** — meaningless when keys are free.
