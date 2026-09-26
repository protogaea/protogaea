# protogaea-watcher

An independent watcher of a Protogaea world (spec §21, stage C). It needs nothing from the operator but the public API, and it trusts nothing it can check:

- **The spark log.** Every two seconds it reads the open window's tree head: the operator's signature must hold, and each head must extend the last one it saw (it verifies the consistency proof itself). A log that shrinks or is rewritten is an alarm: a receipt given earlier could otherwise be dropped unseen.
- **The signed headers.** Each epoch header must match its hash, carry the operator's signature and chain to the previous header; the final tree head it commits to must extend every head seen during the window, so the operator cannot drop confirmed sparks when it closes the epoch.
- **The world.** It replays the world from genesis (the seed and the rules from the API) with the miracles the server logged, and every epoch's `state_root` must equal the one in the log, the signed header where there is one. After the first divergence it stops replaying, as later epochs cannot be compared.

```sh
cargo build --release -p protogaea-watcher
protogaea-watcher --server http://127.0.0.1:8081 --report alarms.jsonl   # follow, alarms to a file
protogaea-watcher --server http://127.0.0.1:8081 --once                  # check once: exit 0 if all holds
```

The server's password, if it has one, comes from `PROTOGAEA_USER` and `PROTOGAEA_PASSWORD`. Each alarm is printed once and appended to the report as a JSON line.

**A world older than its rules' format.** `ruleset_id` is the hash of the rules as they serialize; a world born before the rules gained a field (such as `miracles`) keeps the id it was born with. If genesis from the seed and today's rules does not give the logged root of epoch 0, the watcher tries the id the world declares, says so, and checks everything after it as usual.

**Tested against a cheating operator** on a local world: a signed header whose state root was changed (the hash and the signature fail, and the replay disagrees), a miracle removed from the log (the replay disagrees at its epoch), and half of an epoch's confirmed sparks dropped in the middle of its window (the log shrank from 36 to 18). An honest world passes.

Not checked yet: the ledger root (it needs the spark log's leaves, not served yet), and the time at which a final tree head first appears relative to the beacon round.

Licensed under Apache-2.0, like the core, so that anyone can run and change a watcher.
