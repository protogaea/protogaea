#!/usr/bin/env python3
"""The Protogaea Telegram bot (stage B5): the morning digest and the news of followed clades.

Standard library only. It reads the world server's API and talks to Telegram; it keeps its
subscribers in a SQLite file of its own. Configuration comes from the environment:

  TELEGRAM_TOKEN       the bot's token from @BotFather (required)
  PROTOGAEA_API        the world server, by default http://127.0.0.1:8081
  PROTOGAEA_USER       the server's user, by default `protogaea`
  PROTOGAEA_PASSWORD   the server's password, if it has one
  PROTOGAEA_VIEWER     the viewer's public address for links, by default PROTOGAEA_API + /app/
  BOT_DATA             the bot's database, by default bot.sqlite
  DIGEST_HOUR_UTC      the hour of the morning digest, by default 7 (10:00 in Moscow)
  TELEGRAM_PROXY       an HTTP proxy for api.telegram.org, where Telegram is blocked

  python3 protogaea_bot.py          run the bot
  python3 protogaea_bot.py preview  print today's digest and the latest stories without Telegram
"""
import base64
import html
import json
import os
import sqlite3
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

# ---------------------------------------------------------------- texts

TEXTS = {
    "ru": {
        "lang": "ru",
        "start": (
            "<b>Протогея</b> — мир, который живёт сам по себе: организмы рождаются, делятся на клады, "
            "переселяются и вымирают, а континенты расходятся.\n\n"
            "Каждое утро я пришлю, что случилось за сутки. А если подписаться на кладу, расскажу, "
            "когда с ней что-то произойдёт.\n\n"
            "/digest — что было с прошлой сводки\n"
            "/follow <i>имя или номер</i> — следить за кладой\n"
            "/unfollow <i>имя или номер</i> — перестать\n"
            "/list — за кем я слежу\n"
            "/mute, /unmute — утренняя сводка\n"
            "/stop — забыть меня\n\n"
            "Мир: {viewer}"
        ),
        "digest_title": "<b>Пока тебя не было</b> · день {from_day} → {to_day}",
        "population": "Население: {then}{now}",
        "leader": "Лидер: {then}{now}",
        "counts": "Получили имя: {named} · вымерли: {extinct} · перешейков закрылось: {bridges}",
        "stories": "<b>Что произошло</b>",
        "no_stories": "Больших историй за это время не было.",
        "open": "Открыть мир",
        "followed": "Слежу за {clade}. Сейчас в ней {living}.",
        "followed_extinct": "{clade} уже вымерла, следить не за кем.",
        "unfollowed": "Больше не слежу за {clade}.",
        "not_found": "Не нашёл такую кладу. Напиши номер или имя, например: /follow Ruminans frondosus",
        "follow_usage": "Напиши, за кем следить: /follow <i>имя или номер клады</i>",
        "list_empty": "Ты ни за кем не следишь. Попробуй /follow и имя клады.",
        "list": "<b>Ты следишь за:</b>",
        "list_row": "{clade}: {living}",
        "list_row_extinct": "{clade}: вымерла",
        "muted": "Утренней сводки не будет. Вернуть: /unmute",
        "unmuted": "Утренняя сводка снова включена.",
        "stopped": "Забыл тебя. Вернуться: /start",
        "unknown": "Не понял. Список команд: /start",
        "clade": "клада {id}",
        "continent": "континент {n}",
        "named": "{clade} получила имя: в ней {living}.",
        "extinct": "{clade} вымерла. На пике в ней было {peak}.",
        "dominant": "{clade} стала самой многочисленной кладой мира ({pct}%).",
        "dominant_lost": "{clade} больше не самая многочисленная: теперь это {other}.",
        "story": {
            "comeback": ["Возвращение", "{clade} была на грани: {low}. Теперь их снова {living}."],
            "crossing": ["Переправа", "{clade} закрепилась на другом берегу, {to}: {living}."],
            "invasion": ["Вторжение", "{clade} пришла с другого континента и стала главной, {plate}, потеснив {other}."],
            "arms_race": ["Гонка вооружений", "Охотники стали опаснее ({h0} → {h1}), а их добыча крепче ({d0} → {d1})."],
            "last_of_its_kind": ["Последний из рода", "От {clade}, где было до {peak_n} особей, осталась одна: {organism}."],
            "changing_of_the_guard": ["Смена лидера", "{clade} теперь самая многочисленная клада мира вместо {other}."],
            "split_by_sea": ["Разделены морем", "Перешеек закрылся, и {clade} оказалась по обе стороны пролива."],
            "fall": ["Падение", "{clade} вымерла. На пике в ней было {peak}."],
        },
    },
    "en": {
        "lang": "en",
        "start": (
            "<b>Protogaea</b> is a world that lives on its own: organisms are born, split into clades, "
            "move and die out, and the continents drift apart.\n\n"
            "Every morning I will send what happened in the last day. Follow a clade, and I will tell "
            "you when something happens to it.\n\n"
            "/digest — what happened since the last digest\n"
            "/follow <i>name or number</i> — follow a clade\n"
            "/unfollow <i>name or number</i> — stop following\n"
            "/list — the clades you follow\n"
            "/mute, /unmute — the morning digest\n"
            "/stop — forget me\n\n"
            "The world: {viewer}"
        ),
        "digest_title": "<b>While you were away</b> · day {from_day} → {to_day}",
        "population": "Population: {then}{now}",
        "leader": "Leader: {then}{now}",
        "counts": "Named: {named} · extinct: {extinct} · land bridges closed: {bridges}",
        "stories": "<b>What happened</b>",
        "no_stories": "No big stories in this time.",
        "open": "Open the world",
        "followed": "Following {clade}. It has {living} now.",
        "followed_extinct": "{clade} is already extinct.",
        "unfollowed": "No longer following {clade}.",
        "not_found": "No such clade. Send its number or name, e.g. /follow Ruminans frondosus",
        "follow_usage": "Which clade? /follow <i>name or number</i>",
        "list_empty": "You follow no clades. Try /follow and a clade's name.",
        "list": "<b>You follow:</b>",
        "list_row": "{clade}: {living}",
        "list_row_extinct": "{clade}: extinct",
        "muted": "No morning digest. To bring it back: /unmute",
        "unmuted": "The morning digest is back on.",
        "stopped": "Forgotten. To come back: /start",
        "unknown": "I did not understand. The commands: /start",
        "clade": "clade {id}",
        "continent": "continent {n}",
        "named": "{clade} got its name: it has {living}.",
        "extinct": "{clade} is extinct. At its peak it had {peak}.",
        "dominant": "{clade} is now the most numerous clade of the world ({pct}%).",
        "dominant_lost": "{clade} is no longer the most numerous: {other} is.",
        "story": {
            "comeback": ["Comeback", "{clade} was down to {low}. Now there are {living} again."],
            "crossing": ["Crossing", "{clade} took hold on {to}: {living}."],
            "invasion": ["Invasion", "{clade} came from another continent and now leads {plate}, pushing aside {other}."],
            "arms_race": ["Arms race", "Hunters grew more dangerous ({h0} to {h1}) and their prey tougher ({d0} to {d1})."],
            "last_of_its_kind": ["The last of its kind", "Of {clade}, once {peak_n} strong, one organism remains: {organism}."],
            "changing_of_the_guard": ["A new leader", "{clade} is now the most numerous clade of the world, taking over from {other}."],
            "split_by_sea": ["Split by the sea", "A land bridge closed with {clade} on both sides of the strait."],
            "fall": ["The fall", "{clade} is gone. At its peak it had {peak}."],
        },
    },
}


def lang_of(code):
    return "ru" if (code or "").startswith(("ru", "uk", "be", "kk")) else "en"


def number(n):
    return f"{n:,}".replace(",", " ")


def organisms(n, t):
    """A count of organisms with the right form of the word."""
    n = int(n or 0)
    if t["lang"] == "en":
        return f"{number(n)} organism" + ("" if n == 1 else "s")
    n10, n100 = n % 10, n % 100
    if n10 == 1 and n100 != 11:
        word = "особь"
    elif 2 <= n10 <= 4 and not 12 <= n100 <= 14:
        word = "особи"
    else:
        word = "особей"
    return f"{number(n)} {word}"


# ---------------------------------------------------------------- the world server


class World:
    def __init__(self, base, user, password, viewer):
        self.base = base.rstrip("/")
        self.viewer = viewer
        self.auth = None
        if password:
            token = base64.b64encode(f"{user}:{password}".encode()).decode()
            self.auth = f"Basic {token}"

    def get(self, path, **query):
        url = self.base + path
        query = {k: v for k, v in query.items() if v is not None}
        if query:
            url += "?" + urllib.parse.urlencode(query)
        req = urllib.request.Request(url)
        if self.auth:
            req.add_header("Authorization", self.auth)
        with urllib.request.urlopen(req, timeout=30) as res:
            return json.load(res)

    def clade_link(self, cid, names, t):
        name = names.get(str(cid)) or t["clade"].format(id=cid)
        label = f"<i>{html.escape(name)}</i>" if str(cid) in names else html.escape(name)
        return f'<a href="{html.escape(self.viewer)}#clade={cid}">{label}</a>'

    def find_clade(self, text):
        """A clade by number or by name (case does not matter; a unique prefix will do)."""
        text = text.strip()
        if text.lstrip("#").isdigit():
            try:
                c = self.get(f"/v0/clades/{int(text.lstrip('#'))}")
                return c
            except urllib.error.HTTPError:
                return None
        want = text.lower()
        rows = self.get("/v0/tree")["clades"]
        named = [(r[0], r[6]) for r in rows if r[6]]
        exact = [cid for cid, name in named if name.lower() == want]
        prefix = [cid for cid, name in named if name.lower().startswith(want)]
        found = exact or (prefix if len(prefix) == 1 else [])
        return self.get(f"/v0/clades/{found[0]}") if found else None


# ---------------------------------------------------------------- telegram


class Telegram:
    def __init__(self, token, proxy=None):
        self.base = f"https://api.telegram.org/bot{token}/"
        handlers = [urllib.request.ProxyHandler({"https": proxy, "http": proxy})] if proxy else []
        self.opener = urllib.request.build_opener(*handlers)

    def call(self, method, http_timeout=40, **params):
        data = json.dumps(params).encode()
        req = urllib.request.Request(self.base + method, data=data, headers={"Content-Type": "application/json"})
        try:
            with self.opener.open(req, timeout=http_timeout) as res:
                return json.load(res).get("result")
        except urllib.error.HTTPError as e:
            # The token is in the URL: report the status and Telegram's description, never the URL.
            try:
                description = json.load(e).get("description", "")
            except Exception:
                description = ""
            raise RuntimeError(f"telegram {method}: {e.code} {description}") from None
        except (urllib.error.URLError, TimeoutError, OSError) as e:
            raise RuntimeError(f"telegram {method}: {type(e).__name__}") from None

    def send(self, chat, text):
        return self.call("sendMessage", chat_id=chat, text=text, parse_mode="HTML", disable_web_page_preview=True)


# ---------------------------------------------------------------- the bot's own data


def open_db(path):
    db = sqlite3.connect(path)
    db.executescript(
        """
        CREATE TABLE IF NOT EXISTS chats (
            chat_id INTEGER PRIMARY KEY,
            lang TEXT NOT NULL,
            digest INTEGER NOT NULL DEFAULT 1,
            last_epoch INTEGER,
            joined INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS follows (
            chat_id INTEGER NOT NULL,
            clade_id INTEGER NOT NULL,
            PRIMARY KEY (chat_id, clade_id)
        );
        CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        """
    )
    return db


def kv_get(db, key, default=None):
    row = db.execute("SELECT value FROM kv WHERE key = ?", (key,)).fetchone()
    return row[0] if row else default


def kv_set(db, key, value):
    db.execute("INSERT OR REPLACE INTO kv (key, value) VALUES (?, ?)", (key, str(value)))
    db.commit()


# ---------------------------------------------------------------- messages


def story_text(world, s, names, t):
    title, template = t["story"].get(s["kind"], [s["kind"], ""])
    d = s.get("data") or {}
    clade = lambda cid: "" if cid is None else world.clade_link(cid, names, t)
    continent = lambda plate: "" if plate is None else t["continent"].format(n=int(plate) + 1)
    tenth = lambda v: f"{(v or 0) / 10:.1f}"
    hunting, defense = d.get("hunting_x10") or [0, 0], d.get("defense_x10") or [0, 0]
    organism = d.get("organism")
    text = template.format(
        clade=clade(s.get("clade_id")),
        other=clade(s.get("other_id")),
        low=organisms(d.get("low"), t),
        living=organisms(d.get("living"), t),
        peak=organisms(d.get("peak"), t),
        peak_n=number(int(d.get("peak") or 0)),
        to=continent(d.get("to")),
        plate=continent(s.get("plate")),
        organism=f'<a href="{html.escape(world.viewer)}#organism={organism}">#{organism}</a>' if organism else "",
        h0=tenth(hunting[0]),
        h1=tenth(hunting[1]),
        d0=tenth(defense[0]),
        d1=tenth(defense[1]),
    )
    return f"<b>{html.escape(title)}.</b> {text}"


def digest_text(world, d, epochs_per_day, t):
    names = d.get("names") or {}
    then, now = d.get("then") or {}, d["header"]
    arrow = lambda a, b, show: f"{show(a)} → " if a is not None and a != b else ""
    lines = [
        t["digest_title"].format(from_day=f"{d['since'] / epochs_per_day:.1f}", to_day=f"{d['now'] / epochs_per_day:.1f}"),
        "",
        t["population"].format(then=arrow(then.get("population"), now["population"], number), now=number(now["population"])),
        t["leader"].format(
            then=arrow(then.get("dominant_clade"), now["dominant_clade"], lambda c: world.clade_link(c, names, t)),
            now=world.clade_link(now["dominant_clade"], names, t),
        ),
        t["counts"].format(
            named=d["counts"].get("clade_named", 0),
            extinct=d["counts"].get("clade_extinct", 0),
            bridges=len(d.get("bridges_closed") or []),
        ),
        "",
    ]
    if d["stories"]:
        lines.append(t["stories"])
        lines += [f"• {story_text(world, s, names, t)}" for s in d["stories"]]
    else:
        lines.append(t["no_stories"])
    lines += ["", f'<a href="{html.escape(world.viewer)}">{t["open"]}</a>']
    return "\n".join(lines)


# ---------------------------------------------------------------- the bot


class Bot:
    def __init__(self, world, telegram, db, digest_hour):
        self.world = world
        self.tg = telegram
        self.db = db
        self.digest_hour = digest_hour
        self.info = world.get("/v0/world")

    def t(self, chat):
        row = self.db.execute("SELECT lang FROM chats WHERE chat_id = ?", (chat,)).fetchone()
        return TEXTS[row[0] if row else "ru"]

    def send(self, chat, text):
        try:
            self.tg.send(chat, text)
        except RuntimeError as e:
            # A chat that blocked the bot is forgotten.
            if " 403 " in str(e):
                self.db.execute("DELETE FROM chats WHERE chat_id = ?", (chat,))
                self.db.execute("DELETE FROM follows WHERE chat_id = ?", (chat,))
                self.db.commit()
            else:
                log(str(e))

    # ------------------------------------------------ commands

    def handle(self, message):
        chat = message["chat"]["id"]
        text = (message.get("text") or "").strip()
        if not text:
            return
        command, _, arg = text.partition(" ")
        command = command.split("@")[0].lower()
        known = self.db.execute("SELECT 1 FROM chats WHERE chat_id = ?", (chat,)).fetchone()
        if command == "/start" or not known:
            lang = lang_of((message.get("from") or {}).get("language_code"))
            self.db.execute(
                "INSERT OR IGNORE INTO chats (chat_id, lang, last_epoch, joined) VALUES (?, ?, ?, ?)",
                (chat, lang, self.info["header"]["epoch"], int(time.time())),
            )
            self.db.commit()
            if command == "/start" or not command.startswith("/"):
                self.send(chat, TEXTS[lang]["start"].format(viewer=html.escape(self.world.viewer)))
                return
        t = self.t(chat)
        if command == "/digest":
            self.send_digest(chat)
        elif command in ("/follow", "/unfollow"):
            if not arg.strip():
                return self.send(chat, t["follow_usage"])
            c = self.world.find_clade(arg)
            if not c:
                return self.send(chat, t["not_found"])
            names = {str(c["id"]): c["name"]} if c.get("name") else {}
            link = self.world.clade_link(c["id"], names, t)
            if command == "/unfollow":
                self.db.execute("DELETE FROM follows WHERE chat_id = ? AND clade_id = ?", (chat, c["id"]))
                self.db.commit()
                return self.send(chat, t["unfollowed"].format(clade=link))
            if c.get("extinct_epoch") is not None:
                return self.send(chat, t["followed_extinct"].format(clade=link))
            self.db.execute("INSERT OR IGNORE INTO follows (chat_id, clade_id) VALUES (?, ?)", (chat, c["id"]))
            self.db.commit()
            self.send(chat, t["followed"].format(clade=link, living=organisms(c["living"], t)))
        elif command == "/list":
            ids = [r[0] for r in self.db.execute("SELECT clade_id FROM follows WHERE chat_id = ?", (chat,))]
            if not ids:
                return self.send(chat, t["list_empty"])
            lines = [t["list"]]
            for cid in ids:
                try:
                    c = self.world.get(f"/v0/clades/{cid}")
                except urllib.error.HTTPError:
                    continue
                names = {str(cid): c["name"]} if c.get("name") else {}
                link = self.world.clade_link(cid, names, t)
                row = t["list_row_extinct"] if c.get("extinct_epoch") is not None else t["list_row"]
                lines.append(row.format(clade=link, living=organisms(c["living"], t)))
            self.send(chat, "\n".join(lines))
        elif command in ("/mute", "/unmute"):
            self.db.execute("UPDATE chats SET digest = ? WHERE chat_id = ?", (int(command == "/unmute"), chat))
            self.db.commit()
            self.send(chat, t["unmuted" if command == "/unmute" else "muted"])
        elif command == "/stop":
            self.send(chat, t["stopped"])
            self.db.execute("DELETE FROM chats WHERE chat_id = ?", (chat,))
            self.db.execute("DELETE FROM follows WHERE chat_id = ?", (chat,))
            self.db.commit()
        else:
            self.send(chat, t["unknown"])

    # ------------------------------------------------ the morning digest

    def send_digest(self, chat):
        info = self.world.get("/v0/world")
        now, per_day = info["header"]["epoch"], info["epochs_per_day"]
        row = self.db.execute("SELECT last_epoch FROM chats WHERE chat_id = ?", (chat,)).fetchone()
        since = row[0] if row and row[0] is not None and row[0] < now else now - per_day
        since = max(0, min(since, now - 12), now - 7 * per_day)
        d = self.world.get("/v0/digest", since=since)
        self.send(chat, digest_text(self.world, d, per_day, self.t(chat)))
        self.db.execute("UPDATE chats SET last_epoch = ? WHERE chat_id = ?", (d["now"], chat))
        self.db.commit()

    def morning(self):
        hour, today = time.gmtime().tm_hour, time.strftime("%Y-%m-%d", time.gmtime())
        if hour != self.digest_hour or kv_get(self.db, "digest_day") == today:
            return
        kv_set(self.db, "digest_day", today)
        for (chat,) in self.db.execute("SELECT chat_id FROM chats WHERE digest = 1").fetchall():
            try:
                self.send_digest(chat)
            except (urllib.error.URLError, OSError, KeyError) as e:
                log(f"digest for a chat failed: {type(e).__name__}")
            time.sleep(0.1)

    # ------------------------------------------------ news of followed clades

    def followers(self):
        rows = self.db.execute("SELECT clade_id, chat_id FROM follows").fetchall()
        out = {}
        for cid, chat in rows:
            out.setdefault(cid, []).append(chat)
        return out

    def news(self):
        followers = self.followers()
        # Events: names given, extinctions, changes of the leader.
        cursor = kv_get(self.db, "event_cursor")
        if cursor is None:
            latest = self.world.get("/v0/events", before=0, limit=1)["events"]
            kv_set(self.db, "event_cursor", latest[0]["id"] if latest else 0)
            return
        for _ in range(20):
            page = self.world.get("/v0/events", cursor=cursor, limit=1000)
            events = page["events"]
            if not events:
                break
            names = page.get("names") or {}
            for e in events:
                self.tell_event(e, names, followers)
            cursor = events[-1]["id"]
            kv_set(self.db, "event_cursor", cursor)
        # Stories about followed clades.
        last = int(kv_get(self.db, "story_id", "-1"))
        since = max(0, self.world.get("/v0/world")["header"]["epoch"] - 2 * self.info["epochs_per_day"])
        page = self.world.get("/v0/stories", since=since, limit=200)
        stories = sorted(page["stories"], key=lambda s: s["id"])
        if last < 0:
            kv_set(self.db, "story_id", stories[-1]["id"] if stories else 0)
            return
        for s in stories:
            if s["id"] <= last:
                continue
            chats = set(followers.get(s.get("clade_id"), [])) | set(followers.get(s.get("other_id"), []))
            for chat in chats:
                self.send(chat, story_text(self.world, s, page.get("names") or {}, self.t(chat)))
            kv_set(self.db, "story_id", s["id"])

    def tell_event(self, e, names, followers):
        kind, cid, d = e["kind"], e.get("clade_id"), e.get("data") or {}
        for chat in followers.get(cid, []):
            t = self.t(chat)
            link = self.world.clade_link(cid, names, t)
            if kind == "clade_named":
                self.send(chat, t["named"].format(clade=link, living=organisms(d.get("living", 20), t)))
            elif kind == "clade_extinct":
                self.send(chat, t["extinct"].format(clade=link, peak=organisms(d.get("peak_living"), t)))
            elif kind == "dominant_changed":
                self.send(chat, t["dominant"].format(clade=link, pct=round(d.get("permille", 0) / 10)))
        if kind == "dominant_changed" and d.get("from") is not None:
            for chat in followers.get(d["from"], []):
                t = self.t(chat)
                self.send(chat, t["dominant_lost"].format(
                    clade=self.world.clade_link(d["from"], names, t), other=self.world.clade_link(cid, names, t)))

    # ------------------------------------------------ the loop

    def run(self):
        offset = int(kv_get(self.db, "update_offset", "0"))
        last_news = 0.0
        log("running")
        while True:
            try:
                updates = self.tg.call("getUpdates", offset=offset, timeout=25, allowed_updates=["message"])
            except RuntimeError as e:
                log(str(e))
                time.sleep(5)
                updates = []
            for u in updates or []:
                offset = u["update_id"] + 1
                kv_set(self.db, "update_offset", offset)
                if "message" in u:
                    try:
                        self.handle(u["message"])
                    except (urllib.error.URLError, OSError, KeyError, ValueError) as e:
                        log(f"a command failed: {type(e).__name__}: {e}")
            if time.time() - last_news > 60:
                last_news = time.time()
                try:
                    self.news()
                    self.morning()
                except (urllib.error.URLError, OSError, KeyError, ValueError) as e:
                    log(f"news failed: {type(e).__name__}: {e}")


def log(message):
    print(time.strftime("%H:%M:%S"), message, flush=True)


def world_from_env():
    api = os.environ.get("PROTOGAEA_API", "http://127.0.0.1:8081")
    return World(
        api,
        os.environ.get("PROTOGAEA_USER", "protogaea"),
        os.environ.get("PROTOGAEA_PASSWORD"),
        os.environ.get("PROTOGAEA_VIEWER", api.rstrip("/") + "/app/"),
    )


def main():
    world = world_from_env()
    if sys.argv[1:] == ["preview"]:
        info = world.get("/v0/world")
        now, per_day = info["header"]["epoch"], info["epochs_per_day"]
        for lang in ("ru", "en"):
            d = world.get("/v0/digest", since=max(0, now - per_day))
            print(digest_text(world, d, per_day, TEXTS[lang]), end="\n\n")
        return
    token = os.environ.get("TELEGRAM_TOKEN")
    if not token:
        sys.exit("TELEGRAM_TOKEN is not set")
    db = open_db(os.environ.get("BOT_DATA", "bot.sqlite"))
    telegram = Telegram(token, os.environ.get("TELEGRAM_PROXY"))
    Bot(world, telegram, db, int(os.environ.get("DIGEST_HOUR_UTC", "7"))).run()


if __name__ == "__main__":
    main()
