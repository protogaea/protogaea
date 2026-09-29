# protogaea-server

The world server of stage B1: it runs the authoritative world on a timer, keeps the event log and snapshots, and serves the read API of [spec §23](../docs/spec/spec-v0.2.md#23-api-v0). It replaces the stage A live preview (`protogaea-harness live`). Sparks, wishes and the beacon come in stages B′ and C; until then the epoch seed uses the stand-in beacon of `protogaea_core::run`, like the harness.

```
cargo run --release -p protogaea-server -- --seed 5 --data runs/server --listen 127.0.0.1:8080
```

| Option | Default | Meaning |
|---|---|---|
| `--seed N` | 1 | the seed of a new world; a saved world keeps its own |
| `--data DIR` | `runs/server` | the world, snapshots and the database |
| `--listen ADDR` | `127.0.0.1:8080` | the HTTP address |
| `--epoch-seconds S` | 300 | one epoch every S seconds: windows close on the wall-clock grid of S (with 300, at :00, :05, :10…) and stay open at least 2/5 of S, so a late header moves the close to the next grid point; 0 runs as fast as it can |
| `--archive-every K` | 36 | keep a snapshot every K epochs for proofs and the time machine |
| `--ruleset FILE` | the default ruleset | the rules of a new world |
| `--viewer DIR` | — | serve the viewer's static files at `/app/` |

`PROTOGAEA_PASSWORD` (and `PROTOGAEA_USER`, by default `protogaea`) puts everything except `/health` behind HTTP Basic authentication.

## What it records

Each epoch `n` (the state after `n` epochs; 0 is genesis) is written to SQLite in one transaction:

- **the header:** `state_root` and its subtree roots, the season phase, the population by archetype, clades, the dominant clade, births, deaths by cause and natural events;
- **events:** clades founded, named (reaching 20 living) and extinct, a change of the dominant clade (compared once per world hour), land bridges closing, wildfires, droughts and floods with their place, plague, revivals and season phases. the story detectors (stage B4) are built on these;
- **organisms:** every organism that ever lived, with its parent, clade, genome, birth epoch and, once dead, the epoch, cause, age, energy and cell;
- **clades:** parent, founding and extinction epochs, living and peak counts, reference genome, and a binomial name (spec §7) given once when the clade first reaches the naming threshold: the genus from its dominant trait, the epithet from its preferred habitat, picked by how many clades of that combination were named before;
- **the Muller plot:** clade counts once per world hour;
- **stories:** what the story detectors of `protogaea-stories` found, with a protagonist, the continent and a score. The detectors' memory is saved in the snapshot, so a restart neither loses nor repeats a story.

After the database, `latest.json` is replaced atomically, and every `--archive-every` epochs a copy goes to `snapshots/`. If the server stops between the two writes, the restart drops the database rows past the snapshot and recomputes those epochs; the world is deterministic, so they come out the same (tested by `a_restart_after_a_crash_matches_an_uninterrupted_run`).

Nothing the server records is part of consensus (spec §22): all of it can be recomputed from the genesis, the rules and the world.

## The read API

| Endpoint | Returns |
|---|---|
| `GET /v0/world` | the world, its latest header and when the next epoch is due |
| `GET /v0/ruleset`, `GET /v0/rifts` | the rules; the rift schedule fixed at genesis |
| `GET /v0/map` | the latest state for the map: one array per field (biome, food, moisture, rift phase; organisms' id, cell, clade, hue, archetype, energy, age; active effects) |
| `GET /v0/epochs?from=&to=&step=&limit=`, `GET /v0/epochs/{n}` | headers |
| `GET /v0/events?cursor=&limit=&kind=&clade=` | events after a cursor (an event id), oldest first; `next` continues. With `before=` (an id, 0 for the latest) the newest first instead, and `until=` keeps them to epochs up to a past one |
| `GET /v0/clades?living=&named=&limit=`, `GET /v0/clades/{id}` | clades; a clade with its child clades and population history |
| `GET /v0/organisms/{id}` | an organism, living or dead, with its offspring |
| `GET /v0/museum` | extinct named clades, most recent first |
| `GET /v0/muller?from=&step=` | `[epoch, clade, living]` rows, one sample per world hour, small clades counted with their nearest named ancestor |
| `GET /v0/stories?since=&until=&limit=` | the stories the detectors found between two epochs (by default the world day up to the latest epoch), the best of each clade and kind first, with the names of the clades they are about |
| `GET /v0/digest?since=&until=` | "While you were away" (and, with `until`, any past span, for the chronicle): the header then and now, event counts by kind, the land bridges closed and the best stories since an epoch |
| `GET /v0/replay?from=&step=` | compact frames for the replay (the last world day by default, every second epoch), binary and little-endian: per frame the epoch (u32) and the number of organisms (u32), then per organism the low 32 bits of its id (u32), cell (u16), clade (u32), hue (u16) and archetype (u8). Frames are kept for the last two world days |
| `GET /v0/tree` | the named clades of the season (and the founders), each with its nearest named ancestor as parent, its founding and extinction, peak, hue and name |
| `GET /v0/snapshots`, `GET /v0/snapshots/{epoch}` | archived snapshots |
| `GET /v0/proofs/{epoch}/organism/{id}` | an organism's inclusion proof against the `state_root` of the latest epoch or of an archived one |
| `POST /v0/visits` | an anonymous visit count from the viewer: `{visitor, kind, detail}`, where `visitor` is a random id kept in the browser and `kind` one of `visit`, `digest`, `story`, `card`, `prediction`, `replay`, `view`; at most 300 per visitor an hour |
| `GET /v0/visits/summary` | the measures of the friends test: visitors, returns on day 1 and day 7, the share who opened a story or a card, made a prediction or replayed a day, and visitors by day (UTC) |
| `GET /health` | `ok` |

Visit counts are kept in `visits.sqlite` in the data directory, apart from the world's data: no addresses, no user agents, only the random id, the time and the kind. The summary is for the tests of stage B and must not be public once the world is.

Errors are JSON with a code: `E_NOT_FOUND`, `E_NO_SNAPSHOT`, `E_INTERNAL`.

## Wishes and sparks (stage C)

While the world shows epoch e, the window of epoch e + 1 is open: its challenge commits to the header of e. When the timer fires, the window closes with a final signed tree head, the work of its sparks is added to their wishes, wishes past their lifetime expire and the target moves toward 20,000 sparks an epoch; then the world steps and the next window opens. Sparks are checked in the order of spec §18: format, window, wish, duplicates, rate limits (token buckets: 40 batches per address, refilled at 10 a second; 160 per /24 or /48 subnet, at 40 a second; 40 per miner key, at 10 a second), then one PoW check with the reference C yespower on at most two threads (`E_OVERLOADED` when 512 sparks wait). An invalid PoW bans the key and the address for an hour and ends the check of its batch (the rest are answered `E_BANNED` unchecked). During the first 30 s of a window, a spark that fails it but passes the one just closed is only late (the client had not seen the change yet) and is refused with `E_WINDOW_CLOSED`, without a ban; later it counts as forged, so a forged spark never costs two hashes. `--first-weight H` makes a new log's first window take sparks worth H hashes, for load tests (see [load tests](#load-tests)).

The spark log lives in `sparks.sqlite` in the data directory, apart from the world's database, and the operator's key in `operator.key` (made on first start).

**The ledger (spec §19).** When a window closes, a wish whose work covers `P_E × price_mult / 100` becomes ready (`weather` 100, `migrate` 120, `revive` 200). Ready wishes are ranked by the share of the price they cover (compared by cross-multiplication), ties broken by `BLAKE3("PROTOGAEA/TIEBREAK/V0" ‖ beacon ‖ proposal_id)`, and up to three that do not conflict are selected; a conflicting one waits. The price then rises by an eighth if ready wishes remain, falls by an eighth (not below `--price-min`, 2,900,000 by default) if fewer than three were selected, and holds otherwise. The world applies the selected miracles as it steps into the epoch: an applied one is `executed`; one refused for a reason that passes by itself (an active effect, a cooldown, a crowded start) goes back to the queue; one refused for good is `invalidated` with the reason, and its work is burned. When a window opens, open and queued wishes that can no longer apply for good are invalidated too. Every miracle given to the world is logged with its outcome, so the time machine and any watcher can replay the epoch; after a crash the miracles past the restored snapshot are undone with the world.

**The beacon.** With `--beacon drand` (the default) each epoch's randomness is drand's quicknet: when a window closes, its round is the first one at least 10 s later; the loop waits for it (there is no fallback seed: while drand cannot be reached, the epoch waits), checks its BLS signature, lets the ledger break ties with it, and seeds the step with it and the previous signed header's hash. `--beacon stand-in` derives the beacon from the seed, for offline runs. A world that started with the stand-in switches to drand from its next epoch; its earlier headers stay as they were.

**Signed headers.** After each step the epoch is sealed: its header (protocol §9) chains to the previous one, commits to the world's `state_root`, the ledger (the price and every open or queued wish with its work), the final tree head of the epoch's spark log, the beacon and the miracles given to the epoch, and is signed with the operator key. The next window's challenge commits to the header's hash.

Wish statuses: `open`, `ready` (queued), `selected` (during the step), `executed`, `expired` (an open wish past its lifetime; queued ones do not expire), `invalidated`.

| Endpoint | What it returns |
|---|---|
| `GET /v0/operator` | the operator's public key, which signs tree heads |
| `GET /v0/window` | the open window: epoch, challenge, target (and a spark's weight), world and ruleset ids, when it closes, the latest tree head |
| `POST /v0/proposals` | a wish with its first spark: `{wish, signature, spark}` in hex; answers the `proposal_id` and the spark's receipt |
| `GET /v0/proposals?status=&limit=` | wishes with their status and accumulated work |
| `POST /v0/sparks` | a batch of up to 64 sparks, 72 bytes each (`application/octet-stream`); a receipt or an error for each |
| `GET /v0/ledger` | the price of a miracle for the next selection, its floor, the multipliers and the miracles per epoch; with patrons (spec v0.3, draft) `?clade=ID` adds the clade's share multipliers for help, `cure` and harm (`null` where the share rule closes them) |
| `GET /v0/miracles?from=&to=` | the miracles given to the world by epoch, as the core applies them, with their outcomes |
| `GET /v0/headers?from=&to=`, `GET /v0/headers/{epoch}` | signed epoch headers: the chain of header hashes, `state_root`, `ledger_root`, the epoch's final tree head, the beacon, `miracles_root`, the hash and the operator's signature |
| `GET /v0/sth?epoch=` | the latest signed tree head of an epoch's spark log (the final one once its window closed) |
| `GET /v0/log/{epoch}` | an epoch's whole spark log for watchers: its window's challenge, target and accepted count, and every spark in log order |
| `GET /v0/log/{epoch}/inclusion?index=&size=`, `GET /v0/log/{epoch}/consistency?first=&second=` | inclusion and consistency proofs in the epoch's log |

## Load tests

Run with the load test of the spark client ([`spark/examples/load.rs`](../spark/examples/load.rs)) on the test server (AMD Ryzen 5 5500, 6 cores, 12 threads), against a server on the same machine with `--beacon stand-in` (2026-09-27):

| Test | Result |
|---|---|
| 70 honest sparks a second (20,000 an epoch, the target of spec §19), 8 clients, batches of 32, a spark worth 2 hashes | all accepted; latency p50 292 ms, p99 605 ms; 0.36 cores on average |
| 300 a second | 296 accepted a second; p50 429 ms, p99 680 ms; 1.74 cores. 1.7% refused `E_WINDOW_CLOSED`: sent as the window closed |
| 800 a second offered | 314 accepted a second: the ceiling of the two PoW threads (1.96 cores), about 4.5 times the target; p99 756 ms |
| Closing a window with 19,265 sparks, then the step | 89 ms |
| Spec §29: 10,000 forged sparks a second from 1,000 addresses (each its own /24), with 4 honest miners at the real target (256 hashes a spark) | every honest spark accepted, p50 20 ms, p99 90 ms; the server used 0.25 cores on average, 1.6 at the peak of the first second. Each address had one batch checked (one hash: the first forged spark ends it) and was banned: 1,000 answers of 200, then 69,146 of 403 |
| The same flood from 4 subnets of 250 addresses | the same, and 228 requests refused by the subnet limit (429) |

**What it leaves open.**
- A flood from fresh addresses for every request (a large IPv6 range) is not stopped by bans: each request costs a hash, and at about 600 hashes a second the two threads and their queue fill, so honest sparks would get `E_OVERLOADED` too. The subnet limit bounds it per /48; a queue that serves keys with accepted sparks first is a candidate.
- ~~The spark log grew about 190 bytes a spark~~ (a row with the spark id, its weight and two indexes: 1.1 GB a day at the target). **Now packed:** see [the spark log on disk](#the-spark-log-on-disk).

## The spark log on disk

The open window's sparks are rows (`sparks`, without the spark id or the weight: the id follows from the spark and the epoch, the weight from the window's target). When the window closes they are packed into one blob per epoch (`logs`, [`src/packed.rs`](src/packed.rs)), and the tree heads signed along the way are dropped (each receipt carries its own; the final head stays). The packing:
- wish ids and miner keys (32 bytes each) are kept once for all epochs in a dictionary (`dict`); a blob names each by a short number, once;
- sparks come in runs from one miner for one wish (a batch): a run names them once;
- each thread of a miner tries nonces one after another, so a nonce is written as the step from the last nonce of its thread (its lane), and the lane and the step share one varint; a nonce far from every lane opens a new one (8 bytes). Everything that reads a closed log (`/v0/log/{epoch}`, the proofs, a restart that reopens a window after the world rolled back) reads the blob; the answers are the same.

Measured on the test server: **2.2–2.4 bytes a spark** at the real target (256 hashes a spark), 1.1–1.3 at 2 hashes a spark with 16 or 200 miners, against about 190 as rows and 10 with the first packing (the nonce written out). The step grows with the weight of a spark, so a harder target costs a byte or so more: about 2–3.5 bytes a spark. At 20,000 sparks an epoch that is some 13–20 MB a day and 0.5–0.9 GB a season, instead of 46 GB; the file adds that over a constant of a few MB for the open window's rows. Random nonces, each spark from another miner, are the worst case: about 12 bytes a spark. The first packing (version 1) is still read. A log from before packing is converted on start (closed epochs packed, the file compacted). The watcher replays the packed logs: all holds.

## The Telegram bot

[`bot/`](bot/README.md): the morning digest and the news of followed clades, from this API. The same directory has [patron bots](bot/README.md#patron-bots-for-a-test-world), which play a test world on the v0.3 draft rules through the spark protocol.

## License

The world server is licensed under the GNU Affero General Public License v3.0 only ([LICENSE](LICENSE)); the rest of the repository is under Apache-2.0 ([LICENSING.md](../LICENSING.md)).

## Building

The server needs a C compiler for the bundled SQLite — on Windows either the MSVC toolchain, or the GNU toolchain with MSYS2's `gcc` and `dlltool` on `PATH` (`C:\msys64\ucrt64\bin`) — so a plain `cargo build` at the workspace root leaves it out (`default-members`); build it with `-p protogaea-server`. CI builds and tests it on Linux. A systemd unit is in [`deploy/protogaea-server.service`](../deploy/protogaea-server.service).
