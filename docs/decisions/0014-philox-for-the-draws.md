# 0014. Philox4x32-10 for the random draws

- **Status:** Accepted
- **Date:** 2026-09-29
- **Specification:** §14
- **Amends:** [0003](0003-counter-based-randomness.md) (the function only; randomness stays counter-based)

## Context

[0003](0003-counter-based-randomness.md) made every random number one BLAKE3 hash of `(seed, tick, subject, purpose, k)`. Profiling one world day on a Ryzen 5 5500 showed those hashes taking about 60% of the core's time once the movement code was made faster. BLAKE3 is a cryptographic hash, but the draws need no secrecy: every draw derives from the epoch seed, and the epoch seed from the beacon, which everyone learns at the same moment; after that anyone can replay the world. The draws need statistical quality, the same bits on every platform (native and WebAssembly), and direct access by counter.

No public season has started, so a change of the draws breaks no season's promise.

## Decision

The draws use Philox4x32-10, a counter-based generator of the Random123 family (Salmon et al., SC11):

```
key            = BLAKE3("PROTOGAEA/RAND/V1" ‖ seed)[0..8]           as two u32, little-endian, once per seed
rand(t, p, s, k) = w0 | w1 << 32,  (w0, w1, _, _) = Philox4x32-10([t, s_lo, s_hi, p | k << 8], key)
```

`purpose < 256` and `k < 2^24`. The key still comes from BLAKE3, so the seed stays the only source of the world's randomness. The implementation is checked against Random123's known-answer vectors, and one draw is fixed as a test vector in the core.

## Consequences

- One draw costs about 11.6 ns instead of 134 ns natively (11.5×) and 16 ns instead of 177 ns in WebAssembly (11×). A world day on one thread takes 3.9 s instead of 7.9 s under the Season 1 rules.
- Every world changes: state roots, maps and the test worlds. The live test world restarts.
- The generator is not a cryptographic PRF. That is not needed here (see the context), and Philox4x32-10 passes the BigCrush battery with rounds to spare (7 rounds pass; 10 is the standard).
- The properties of [0003](0003-counter-based-randomness.md) hold: no state, results independent of call order, honest counterfactuals.

## Alternatives considered

- **Keep BLAKE3.** The safest choice, but it dominates the cost of a tick.
- **Threefry4x64-20 or -13.** A natural fit for 64-bit counters and keys, 4.4–6.9× faster than BLAKE3, but slower than Philox4x32 natively and in WebAssembly.
- **Philox4x64-10.** As fast as Philox4x32 natively, but WebAssembly has no 64×64→128 multiply, which makes it only 1.6× faster there, and the browser's time machine runs the core in WebAssembly.
- **One BLAKE3 hash per organism and tick, split among its draws.** Fewer hashes, but every call site would have to know its slot in the output, and draws with rejection sampling would not fit.
