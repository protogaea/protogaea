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
| `--epoch-seconds S` | 300 | one epoch every S seconds; 0 runs as fast as it can |
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

While the world shows epoch e, the window of epoch e + 1 is open: its challenge commits to the header of e. When the timer fires, the window closes with a final signed tree head, the work of its sparks is added to their wishes, wishes past their lifetime expire and the target moves toward 20,000 sparks an epoch; then the world steps and the next window opens. Sparks are checked in the order of spec §18: format, window, wish, duplicates, rate limits (a bucket of 40 batches per address, 10 a second), then one PoW check with the reference C yespower on at most two threads (`E_OVERLOADED` when 512 sparks wait). An invalid PoW bans the key and the address for an hour.

The spark log lives in `sparks.sqlite` in the data directory, apart from the world's database, and the operator's key in `operator.key` (made on first start).

**The ledger (spec §19).** When a window closes, a wish whose work covers `P_E × price_mult / 100` becomes ready (`weather` 100, `migrate` 120, `revive` 200). Ready wishes are ranked by the share of the price they cover (compared by cross-multiplication), ties broken by `BLAKE3("PROTOGAEA/TIEBREAK/V0" ‖ beacon ‖ proposal_id)`, and up to three that do not conflict are selected; a conflicting one waits. The price then rises by an eighth if ready wishes remain, falls by an eighth (not below `--price-min`, 2,900,000 by default) if fewer than three were selected, and holds otherwise. The world applies the selected miracles as it steps into the epoch: an applied one is `executed`; one refused for a reason that passes by itself (an active effect, a cooldown, a crowded start) goes back to the queue; one refused for good is `invalidated` with the reason, and its work is burned. When a window opens, open and queued wishes that can no longer apply for good are invalidated too. Every miracle given to the world is logged with its outcome, so the time machine and any watcher can replay the epoch; after a crash the miracles past the restored snapshot are undone with the world.

Wish statuses: `open`, `ready` (queued), `selected` (during the step), `executed`, `expired` (an open wish past its lifetime; queued ones do not expire), `invalidated`.

| Endpoint | What it returns |
|---|---|
| `GET /v0/operator` | the operator's public key, which signs tree heads |
| `GET /v0/window` | the open window: epoch, challenge, target (and a spark's weight), world and ruleset ids, when it closes, the latest tree head |
| `POST /v0/proposals` | a wish with its first spark: `{wish, signature, spark}` in hex; answers the `proposal_id` and the spark's receipt |
| `GET /v0/proposals?status=&limit=` | wishes with their status and accumulated work |
| `POST /v0/sparks` | a batch of up to 64 sparks, 72 bytes each (`application/octet-stream`); a receipt or an error for each |
| `GET /v0/ledger` | the price of a miracle for the next selection, its floor, the multipliers and the miracles per epoch |
| `GET /v0/miracles?from=&to=` | the miracles given to the world by epoch, as the core applies them, with their outcomes |
| `GET /v0/sth?epoch=` | the latest signed tree head of an epoch's spark log (the final one once its window closed) |
| `GET /v0/log/{epoch}/inclusion?index=&size=`, `GET /v0/log/{epoch}/consistency?first=&second=` | inclusion and consistency proofs in the epoch's log |

## The Telegram bot

[`bot/`](bot/README.md): the morning digest and the news of followed clades, from this API.

## License

The world server is licensed under the GNU Affero General Public License v3.0 only ([LICENSE](LICENSE)); the rest of the repository is under Apache-2.0 ([LICENSING.md](../LICENSING.md)).

## Building

The server needs a C compiler for the bundled SQLite — on Windows either the MSVC toolchain, or the GNU toolchain with MSYS2's `gcc` and `dlltool` on `PATH` (`C:\msys64\ucrt64\bin`) — so a plain `cargo build` at the workspace root leaves it out (`default-members`); build it with `-p protogaea-server`. CI builds and tests it on Linux. A systemd unit is in [`deploy/protogaea-server.service`](../deploy/protogaea-server.service).
