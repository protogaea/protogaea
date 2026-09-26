# protogaea-protocol

The spark protocol v0 ([docs/protocol.md](../docs/protocol.md), spec Part III) in plain Rust, shared by the world server, the spark clients (it builds for WebAssembly) and independent watchers:

- [`wish`](src/wish.rs): the canonical bytes of a wish, strictly decoded so that every wish has one encoding and one `proposal_id`; the author's Ed25519 signature over it, verified strictly (a canonical `S`, no small-order keys);
- [`spark`](src/spark.rs): the epoch challenge, the 72-byte spark and batches of up to 64, `spark_id`, the spark log leaf, the PoW check against the epoch target with [`protogaea-pow`](../pow/README.md), and the target adjustment;
- [`log`](src/log.rs): the spark log, a Merkle tree after RFC 9162 §2.1 (RFC 6962) with BLAKE3, with inclusion and consistency proofs;
- [`sth`](src/sth.rs): signed tree heads and receipts.

```sh
cargo test -p protogaea-protocol
```

The tests pin test vectors (a `proposal_id` and its signature, a challenge and a spark found under an easy target, the roots of small trees), check every inclusion and consistency proof for trees of up to 33 leaves and that altered ones fail, and check the strictness of signature verification.

Licensed under Apache-2.0, like the core.
