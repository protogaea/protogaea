# The Protogaea Telegram bot

The bot of stage B5 ([roadmap](../../docs/roadmap.md)): it sends a morning digest of the world and tells the people who follow a clade when something happens to it. It reads the world server's API and needs nothing but Python 3.10 or newer (standard library only). It is part of the world server and, like it, licensed under AGPL-3.0-only ([LICENSE](../LICENSE)).

## What it does

- **The morning digest**, once a day at `DIGEST_HOUR_UTC`: what changed since the last digest, from `/v0/digest` (population, the leading clade, clades named and extinct, land bridges closed, the best stories), with links into the viewer.
- **Following a clade:** `/follow` with a clade's number or name (a unique beginning of the name will do). The bot then tells when the clade gets its name, becomes or stops being the most numerous, goes extinct, or is in a story.
- Commands: `/start`, `/digest`, `/follow`, `/unfollow`, `/list`, `/mute`, `/unmute`, `/stop`. Messages are in Russian for Russian, Ukrainian, Belarusian and Kazakh Telegram settings, and in English otherwise.

A chat that blocks the bot is forgotten. The bot keeps only chat ids, their language, the clades they follow and when they last had a digest.

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
