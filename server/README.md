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
- **events:** clades founded, named (reaching 20 living) and extinct, a change of the dominant clade (compared once per world hour), land bridges closing, wildfires, droughts and floods with their place, plague, revivals and season phases. Story detectors (stage B5) will be built on these;
- **organisms:** every organism that ever lived, with its parent, clade, genome, birth epoch and, once dead, the epoch, cause, age, energy and cell;
- **clades:** parent, founding and extinction epochs, living and peak counts, reference genome;
- **the Muller plot:** clade counts once per world hour.

After the database, `latest.json` is replaced atomically, and every `--archive-every` epochs a copy goes to `snapshots/`. If the server stops between the two writes, the restart drops the database rows past the snapshot and recomputes those epochs; the world is deterministic, so they come out the same (tested by `a_restart_after_a_crash_matches_an_uninterrupted_run`).

Nothing the server records is part of consensus (spec §22): all of it can be recomputed from the genesis, the rules and the world.

## The read API

| Endpoint | Returns |
|---|---|
| `GET /v0/world` | the world, its latest header and when the next epoch is due |
| `GET /v0/ruleset`, `GET /v0/rifts` | the rules; the rift schedule fixed at genesis |
| `GET /v0/map` | the latest state for the map: one array per field (biome, food, moisture, rift phase; organisms' id, cell, clade, hue, archetype, energy, age; active effects) |
| `GET /v0/epochs?from=&to=&step=&limit=`, `GET /v0/epochs/{n}` | headers |
| `GET /v0/events?cursor=&limit=&kind=&clade=` | events after a cursor (an event id), oldest first; `next` continues |
| `GET /v0/clades?living=&named=&limit=`, `GET /v0/clades/{id}` | clades; a clade with its child clades and population history |
| `GET /v0/organisms/{id}` | an organism, living or dead, with its offspring |
| `GET /v0/museum` | extinct named clades, most recent first |
| `GET /v0/muller?from=&step=` | `[epoch, clade, living]` rows, one sample per world hour |
| `GET /v0/snapshots`, `GET /v0/snapshots/{epoch}` | archived snapshots |
| `GET /v0/proofs/{epoch}/organism/{id}` | an organism's inclusion proof against the `state_root` of the latest epoch or of an archived one |
| `GET /health` | `ok` |

Errors are JSON with a code: `E_NOT_FOUND`, `E_NO_SNAPSHOT`, `E_INTERNAL`.

## Building

The server needs a C compiler for the bundled SQLite and, on Windows, the MSVC toolchain, so a plain `cargo build` at the workspace root leaves it out (`default-members`); build it with `-p protogaea-server`. CI builds and tests it on Linux. A systemd unit is in [`deploy/protogaea-server.service`](../deploy/protogaea-server.service).
