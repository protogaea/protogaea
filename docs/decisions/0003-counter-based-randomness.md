# 0003. Counter-based randomness

- **Status:** Accepted
- **Date:** 2026-09-24
- **Specification:** §14

## Context

The simulation needs a great deal of randomness: queue order, mutations, attacks, placement, natural events. A conventional stateful generator makes every result depend on the exact order of calls. That makes parallel computation unsafe and counterfactuals meaningless: remove one miracle, and every later random number shifts, so the "world without the miracle" differs by pure noise.

## Decision

The generator has no state. Every random number is computed directly:

```
rand(purpose, subject, k) = u64_le(BLAKE3("PROTOGAEA/RAND/V0" ‖ epoch_seed ‖ tick ‖ subject ‖ purpose ‖ k)[0..8])
```

Uniform values in `[0, n)` are drawn by rejection sampling with an increasing `k`. The epoch seed comes from the beacon, never from proof-of-work results.

## Consequences

- Results do not depend on call order, so work within a tick can run in parallel, as long as conflicts are resolved deterministically.
- Counterfactuals are honest: every other organism gets exactly the same "luck", so a difference between the worlds is the effect of the miracle.
- There is no generator state to store, serialize or hash.
- Every call site needs a unique `purpose` code, which has to be managed carefully.

## Alternatives considered

- **A stateful PRNG** (for example, PCG or Xoshiro) — faster per call, but it loses all three properties above.
