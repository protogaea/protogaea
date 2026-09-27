# protogaea-spark

The spark client (stage C): a naturalist's key, wishes and the sparks that carry them, from the command line. It is the first form of the desktop client of spec §8.

```sh
cargo build --release -p protogaea-spark --features c     # with the reference C yespower, faster
protogaea-spark key                                         # make a key, or show it
protogaea-spark wish weather X Y rain|drought               # a wish with its first spark
protogaea-spark wish migrate CLADE FROM_X FROM_Y TO_X TO_Y
protogaea-spark wish revive museum|spores ENTRY X Y [i:j ...]   # edit steps move a point from trait i to j
protogaea-spark mine PROPOSAL_ID --threads 4 --minutes 10   # sparks for a wish
```

Options: `--server URL` (default `http://127.0.0.1:8081`), `--key FILE` (default `protogaea-spark.key`, made on first use; it is the naturalist's identity), `--threads N` (default half the logical CPUs). The server's password, if it has one, comes from `PROTOGAEA_USER` and `PROTOGAEA_PASSWORD`.

- A wish is signed with the key and sent together with its first spark, as spec §17 requires.
- Sparks are mined on several threads for the open window and sent in batches of up to 64 every few seconds; when the window changes, sparks for the old one are dropped.
- **Every receipt is checked** against the operator's key: the spark must be in a tree head the operator signed. A receipt that does not check stops the client.
- All three actions can be wished for: `weather`, `migrate` (a clade, the center of its 5 × 5 source area and a target) and `revive` (a museum clade id or a spore bank index, a start, and up to two edit steps).

## The load test

`examples/load.rs` loads a server's spark intake: honest miners that mine real sparks for a wish, and a flood of random nonces (almost all forged) from many addresses, each client bound to its own loopback address (Linux binds any `127.x.y.z`), so the server's limits per address and subnet tell them apart. It reports sparks accepted a second, the honest latency, the answers by code and, with `--pid`, the server's CPU.

```
cargo run --release -p protogaea-spark --features c --example load -- --server 127.0.0.1:8095   --proposal ID --honest 8 --honest-rate 300 --batch 32 --flood-addrs 1000 --flood-rate 10000 --seconds 60 --pid PID
```

The results are in the [server README](../server/README.md#load-tests).

Licensed under AGPL-3.0-only, like the rest of the applications ([LICENSE](LICENSE)).
