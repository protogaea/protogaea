# protogaea-pow

The proof of work behind sparks (spec [§16](../docs/spec/spec-v0.2.md#16-pow-algorithm-and-hardware) and [§18](../docs/spec/spec-v0.2.md#18-sparks-format-intake-receipts)): yespower 1.0 with `N = 2048`, `r = 32` (8 MiB of memory per thread) and the personalization `PROTOGAEA/SPARK/V0`, the 146-byte spark input, the target check and a spark's weight in work units.

```sh
cargo test -p protogaea-pow                                   # the reference vectors of yespower 1.0
cargo run --release -p protogaea-pow --example bench -- 10    # hash rates and the cost of a check
PROTOGAEA_POW_NATIVE=1 cargo run --release -p protogaea-pow --example bench   # with -march=native
```

It needs a C compiler (on Windows, MSYS2's gcc with the GNU toolchain).

## yespower

[`yespower/`](yespower/) is the reference implementation by Alexander Peslyak (Openwall), [github.com/openwall/yespower](https://github.com/openwall/yespower) at commit `1977c283bc43eed5a2c2579e02d6d996e49866b0`, unchanged. It is under a 2-clause BSD license (see the file headers), which allows its use here next to Apache-2.0 and AGPL-3.0 code. The build is portable: no CPU-specific flags, so one binary runs on every x86-64 and ARM64 machine (spec §16); SIMD comes only from the compiler's baseline.

The tests check the reference `TESTS-OK` vectors of yespower 1.0, including one with a personalization string.

## Measurements (2026-09-26)

Hash rates of `yespower(N = 2048, r = 32, "PROTOGAEA/SPARK/V0")` over the spark input, 5 s per measurement; a spark check is a fresh 8 MiB allocation and one hash.

| CPU | Build | 1 thread | Half the threads | All threads | Checking one spark (p50) |
|---|---|---|---|---|---|
| AMD Ryzen 7 8745HS, 8 cores / 16 threads, 16 MiB L3, Windows | portable | 282 H/s | 1,399 H/s (8) | 1,798 H/s (16) | 4.3 ms |
| | `-march=native` | 299 H/s | 1,638 H/s (8) | 1,839 H/s (16) | 3.9 ms |
| AMD Ryzen 5 5500, 6 cores / 12 threads, 16 MiB L3, Linux | portable | 318 H/s | 1,119 H/s (6) | 1,199 H/s (12) | 6.6 ms |
| | `-march=native` | 309 H/s | 1,277 H/s (6) | 1,264 H/s (12) | 6.6 ms |

- **Memory, not arithmetic, sets the pace.** Both CPUs have 16 MiB of L3, room for two threads' 8 MiB; beyond that the threads share memory bandwidth, and the second half of the threads adds little (on the Ryzen 5 5500 six threads do as much as twelve). This is what yespower is designed for.
- **A tuned build gains about 5%.** The portable build is what the spark client ships.
- **Checking a spark costs 4–7 ms** with fresh memory, most of it the 8 MiB allocation on Linux; a server that keeps one hasher per thread pays about 3.2 ms (one hash). At the reference load of 20,000 sparks an epoch (spec §18), 67 a second, that is about a fifth of one core.
- **The price floor.** Spec §19 proposes about 8 core-hours of the reference core as `P_min`. On the Ryzen 5 5500 one thread alone does about 310 H/s, 8.9 million work units in 8 hours; with every thread busy each does about 100 H/s, 2.9 million. Which of the two a "core-hour" means is part of roadmap decision 6.

Not measured yet: ARM64 (no machine at hand) and the spark client in the browser (yespower compiled to WebAssembly). GPUs are not measured: yespower is built for CPUs, and the project does not plan for GPU sparks.
