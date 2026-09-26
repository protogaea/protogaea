# protogaea-spark

The spark client (stage C): a naturalist's key, wishes and the sparks that carry them, from the command line. It is the first form of the desktop client of spec §8.

```sh
cargo build --release -p protogaea-spark --features c     # with the reference C yespower, faster
protogaea-spark key                                         # make a key, or show it
protogaea-spark wish weather X Y rain|drought               # a wish with its first spark
protogaea-spark mine PROPOSAL_ID --threads 4 --minutes 10   # sparks for a wish
```

Options: `--server URL` (default `http://127.0.0.1:8081`), `--key FILE` (default `protogaea-spark.key`, made on first use; it is the naturalist's identity), `--threads N` (default half the logical CPUs). The server's password, if it has one, comes from `PROTOGAEA_USER` and `PROTOGAEA_PASSWORD`.

- A wish is signed with the key and sent together with its first spark, as spec §17 requires.
- Sparks are mined on several threads for the open window and sent in batches of up to 64 every few seconds; when the window changes, sparks for the old one are dropped.
- **Every receipt is checked** against the operator's key: the spark must be in a tree head the operator signed. A receipt that does not check stops the client.
- For now only `weather` wishes can be made from the command line; `migrate` and `revive` come with the miracles in the core.

Licensed under AGPL-3.0-only, like the rest of the applications ([LICENSE](LICENSE)).
