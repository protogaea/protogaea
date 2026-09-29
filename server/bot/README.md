# The Protogaea Telegram bot

The bot of stage B5 ([roadmap](../../docs/roadmap.md)): it sends a morning digest of the world and tells the people who follow a clade when something happens to it. It reads the world server's API and needs nothing but Python 3.10 or newer (standard library only). It is part of the world server and, like it, licensed under AGPL-3.0-only ([LICENSE](../LICENSE)).

## What it does

- **The morning digest**, once a day at `DIGEST_HOUR_UTC`: what changed since the last digest, from `/v0/digest` (population, the leading clade, clades named and extinct, land bridges closed, the best stories), with links into the viewer.
- **Following a clade:** `/follow` with a clade's number or name (a unique beginning of the name will do). The bot then tells when the clade gets its name, becomes or stops being the most numerous, goes extinct, or is in a story.
- **Following a continent:** `/continent` with its number (again to stop). The bot then tells of the stories on it: crossings to it, invasions, the last of a great clade that died there.
- Commands: `/start`, `/digest`, `/follow`, `/unfollow`, `/continent`, `/list`, `/mute`, `/unmute`, `/stop`. Messages are in Russian for Russian, Ukrainian, Belarusian and Kazakh Telegram settings, and in English otherwise.

A chat that blocks the bot is forgotten. The bot keeps only chat ids, their language, the clades and continents they follow and when they last had a digest.

## Running it

```sh
TELEGRAM_TOKEN=... PROTOGAEA_API=http://127.0.0.1:8081 PROTOGAEA_PASSWORD=... \
PROTOGAEA_VIEWER=https://example.org/app/ python3 protogaea_bot.py
python3 protogaea_bot.py preview    # today's digest in both languages, without Telegram
```

| Variable | Meaning |
|---|---|
| `TELEGRAM_TOKEN` | the bot's token from @BotFather (required) |
| `PROTOGAEA_API` | the world server, by default `http://127.0.0.1:8081` |
| `PROTOGAEA_USER`, `PROTOGAEA_PASSWORD` | the server's password, if it has one |
| `PROTOGAEA_VIEWER` | the viewer's public address, for links |
| `BOT_DATA` | the bot's database, by default `bot.sqlite` |
| `DIGEST_HOUR_UTC` | the hour of the morning digest, by default 7 (10:00 in Moscow) |
| `TELEGRAM_PROXY` | an HTTP proxy for `api.telegram.org`, where Telegram is blocked |

A systemd unit is in [`deploy/protogaea-bot.service`](../../deploy/protogaea-bot.service).

## Patron bots for a test world

[`patron_bots.py`](patron_bots.py) plays a test world under the [v0.3 draft](../../docs/spec/spec-v0.3-draft.md) rules while it has no players. Each bot backs clades by a strategy, as the harness's patron bots do: `random` (any help for a random clade), `weak` (the smallest clade that can be helped), `leader` (the largest) and `harass` (harm to the smallest clade that can be harmed). Help is `shelter`, `forage`, `cure`, a gift, or a hybrid with the largest compatible neighbour; a bot asks `/v0/ledger?clade=ID` whether the share rule allows it first.

The bots go through the real protocol: a wish signed with the spark client (`protogaea-spark`) and sent with its first spark, then `PATRON_SECONDS` of mining a bot and epoch on one thread, so a watcher can check all of it. A player's price floor (about 8 core-hours) would take a bot days, so the test world runs with a low `--price-min` (6000 work units: a few wishes an epoch from four bots). Settings: `PROTOGAEA_API`, `PROTOGAEA_USER` and `PROTOGAEA_PASSWORD`, `PROTOGAEA_SPARK` (the client), `STATE_DIRECTORY` (the bots' keys), `PATRON_BOTS` (the strategies) and `PATRON_SECONDS` (20). A systemd unit is in [`deploy/protogaea-patrons.service`](../../deploy/protogaea-patrons.service).
