# 0013. yespower as the candidate proof of work

- **Status:** Proposed — final after benchmarks
- **Date:** 2026-09-24
- **Specification:** §16

## Context

Sparks must be computable on ordinary processors, including ARM64 laptops, desktops and Raspberry Pi, and in the browser through WASM. They must not require JIT compilation or x86-specific instructions. The server verifies every spark, so verification must be cheap enough to fit a budget of about two cores. We also want GPUs to have as small an advantage as practical, and we must describe that honestly.

## Decision

The candidate is **yespower 1.0** with `N = 2048` and `r = 32` — about 8 MiB of memory per thread — and the personalization string `pers = "PROTOGAEA/SPARK/V0"`, so that work for other yespower-based systems is not reusable here. The choice becomes final only after:

- portable benchmarks on x86-64, ARM64 and available GPUs, of both computation and verification;
- a fixed set of test vectors;
- a review of the license of the specific implementation.

The public wording is: CPU-friendly and GPU-unfriendly by design, with no guarantee against ASICs or specialized hardware.

## Consequences

- Verification costs on the order of milliseconds and 8 MiB of memory per spark, which sets the spark target at about 20,000 per epoch across the network.
- Browsers can compute sparks with Web Workers at about 8 MiB per worker, less efficiently than native code.
- yespower is used by some cryptocurrencies, so antivirus products may flag it. Code signing, reproducible builds and advance submission to vendors address this.

## Alternatives considered

- **RandomX** — needs hundreds of megabytes to gigabytes of memory and relies on JIT compilation for good performance; verification is heavier for the server.
- **SHA-256 or BLAKE-based hashcash** — dominated by GPUs and ASICs.
- **A custom memory-hard construction** — unreviewed cryptography is a risk we do not need.
