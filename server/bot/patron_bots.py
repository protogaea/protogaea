#!/usr/bin/env python3
"""Patron bots for a test world (spec v0.3, draft).

Each bot backs clades by a strategy, as the harness's bots do (`harness/src/patrons.rs`), but
through the real protocol: it signs a wish with the spark client, sends it with its first spark,
and every epoch puts a fixed, small amount of mining into it. The sparks are real, so the log,
the ledger and every miracle can be checked by a watcher; the test world's price floor is set
low (`--price-min`), so a bot spends seconds of one core an epoch, not hours.

Strategies: `random` (a random clade, any help), `weak` (the smallest clade that can be helped),
`leader` (the largest), `harass` (harm to the smallest clade that can be harmed). Help is one of
`shelter`, `forage`, `cure`, a gift, or a hybrid with the largest compatible neighbour.

Settings come from the environment:
  PROTOGAEA_API       the world server (http://127.0.0.1:8081)
  PROTOGAEA_USER, PROTOGAEA_PASSWORD   the server's password, if it has one
  PROTOGAEA_SPARK     the spark client (/opt/protogaea/protogaea-spark)
  STATE_DIRECTORY     where the bots' keys live (the current directory)
  PATRON_BOTS         the strategies, one bot each (random,weak,leader,harass)
  PATRON_SECONDS      seconds of mining per bot and epoch, on one thread (20)
"""

import base64
import json
import os
import random
import re
import subprocess
import sys
import time
import urllib.request

API = os.environ.get("PROTOGAEA_API", "http://127.0.0.1:8081").rstrip("/")
SPARK = os.environ.get("PROTOGAEA_SPARK", "/opt/protogaea/protogaea-spark")
STATE = os.environ.get("STATE_DIRECTORY", ".")
BOTS = [b for b in os.environ.get("PATRON_BOTS", "random,weak,leader,harass").split(",") if b]
SECONDS = float(os.environ.get("PATRON_SECONDS", "20"))

GIFTS = ["swim", "venom", "camo", "keen", "hardy", "scavenge"]
HELP = ["shelter", "forage", "cure", "gift", "hybrid"]
HARM = ["blight", "expose", "sickness"]
# The core's candidates (spec v0.3 §4, §5): members around a target, harm's protection.
MIN_MEMBERS, HYBRID_EACH, HYBRID_DISTANCE = 10, 5, (2, 8)
PROTECT_LIVING, PROTECT_PERMILLE = 20, 30
# A wish still waiting after this many epochs is given up (it expires after 288 by itself).
PATIENCE = 72
LIVE = ("open", "ready", "selected")


def log(msg):
    print(msg, flush=True)


def get(path):
    req = urllib.request.Request(API + path)
    password = os.environ.get("PROTOGAEA_PASSWORD")
    if password:
        user = os.environ.get("PROTOGAEA_USER", "protogaea")
        token = base64.b64encode(f"{user}:{password}".encode()).decode()
        req.add_header("Authorization", "Basic " + token)
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.load(r)


def spark(*args):
    """Runs the spark client; returns its exit code and output."""
    cmd = [SPARK, *map(str, args), "--server", API, "--threads", "1"]
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    return p.returncode, (p.stdout + p.stderr).strip()


def distance(a, b):
    """Genome distance in mutation steps, as `core::genome::Genome::distance`."""
    ta = list(a["traits"]) + list(a.get("extra", [0, 0]))
    tb = list(b["traits"]) + list(b.get("extra", [0, 0]))
    return sum(abs(x - y) for x, y in zip(ta, tb)) // 2


class World:
    """What the bots need of the latest state: members and their densest 5 × 5 area per clade."""

    def __init__(self):
        m = get("/v0/map")
        self.epoch = m["epoch"]
        self.width = m["width"]
        o = m["organisms"]
        self.population = len(o["id"])
        self.cells = {}
        for cell, clade in zip(o["cell"], o["clade"]):
            self.cells.setdefault(clade, []).append(cell)
        self.refs = {c["id"]: c["reference"] for c in get("/v0/clades")["clades"] if c["living"] > 0}
        self.best = {}
        for clade, cells in self.cells.items():
            if len(cells) >= MIN_MEMBERS:
                self.best[clade] = self.densest(cells)

    def xy(self, cell):
        return cell % self.width, cell // self.width

    def around(self, cells, center, r=2):
        cx, cy = self.xy(center)
        return sum(1 for c in cells if max(abs(c % self.width - cx), abs(c // self.width - cy)) <= r)

    def densest(self, cells):
        """(members, center) of the 5 × 5 area with the most members."""
        return max((self.around(cells, c), c) for c in set(cells))

    def living(self, clade):
        return len(self.cells.get(clade, []))

    def fits(self):
        """Clades with 10+ members in one area: (living, clade, center)."""
        return sorted((self.living(c), c, cell) for c, (n, cell) in self.best.items() if n >= MIN_MEMBERS)

    def partner(self, clade, center):
        """The largest other clade with 5+ members around `center` and a crossable genome."""
        ref = self.refs.get(clade)
        if ref is None:
            return None
        options = []
        for other, cells in self.cells.items():
            if other == clade or other not in self.refs:
                continue
            n = self.around(cells, center)
            d = distance(ref, self.refs[other])
            if n >= HYBRID_EACH and HYBRID_DISTANCE[0] <= d <= HYBRID_DISTANCE[1]:
                options.append((n, other))
        return max(options)[1] if options else None


class Bot:
    def __init__(self, strategy):
        self.strategy = strategy
        self.key = os.path.join(STATE, f"{strategy}.key")
        self.wish = None  # (proposal id, epoch sent, description)

    def plan(self, w, rng):
        """The wish's spark client arguments and a description, or None."""
        fits = w.fits()
        if not fits:
            return None
        if self.strategy == "harass":
            harmable = [f for f in fits if f[0] >= PROTECT_LIVING and f[0] * 1000 >= PROTECT_PERMILLE * w.population]
            order = harmable
        elif self.strategy == "weak":
            order = fits
        elif self.strategy == "leader":
            order = fits[::-1]
        else:
            order = fits[:]
            rng.shuffle(order)
        for living, clade, center in order[:12]:
            action = rng.choice(HARM) if self.strategy == "harass" else rng.choice(HELP)
            kind = "harm" if action in HARM else ("cure" if action == "cure" else "help")
            share = get(f"/v0/ledger?clade={clade}").get("share_mult", {})
            if share.get(kind) is None:
                continue
            x, y = w.xy(center)
            if action == "hybrid":
                other = w.partner(clade, center)
                if other is None:
                    action = "shelter"
                else:
                    return ["wish", "hybrid", clade, other, x, y], f"hybrid {clade} x {other} at ({x},{y})"
            if action == "gift":
                gift = rng.choice(GIFTS)
                return ["wish", "gift", gift, clade, x, y], f"gift {gift} to {clade} ({living}) at ({x},{y})"
            return ["wish", action, clade, x, y], f"{action} {clade} ({living}) at ({x},{y})"
        return None

    def status(self, statuses):
        if self.wish is None:
            return None
        return statuses.get(self.wish[0])


def main():
    if not os.path.exists(SPARK):
        sys.exit(f"no spark client at {SPARK}")
    bots = [Bot(s) for s in BOTS]
    rng = random.Random()
    last = None
    log(f"patron bots {', '.join(BOTS)} on {API}, {SECONDS:.0f} s of mining each an epoch")
    while True:
        try:
            epoch = get("/v0/window")["epoch"]
        except Exception as e:  # the server restarts, or the network drops
            log(f"window: {e}")
            time.sleep(30)
            continue
        if epoch == last:
            time.sleep(10)
            continue
        last = epoch
        try:
            rows = get("/v0/proposals?limit=1000")["proposals"]
            statuses = {r["id"]: (r["status"], r.get("reason")) for r in rows}
            w = World()
        except Exception as e:
            log(f"epoch {epoch}: reading the world failed: {e}")
            continue
        for bot in bots:
            st = bot.status(statuses)
            if bot.wish and (st is None or st[0] not in LIVE):
                log(f"epoch {epoch}: {bot.strategy}: {bot.wish[2]} -> {st[0] if st else 'unknown'}"
                    + (f" ({st[1]})" if st and st[1] else ""))
                bot.wish = None
            elif bot.wish and epoch - bot.wish[1] > PATIENCE:
                log(f"epoch {epoch}: {bot.strategy}: gives up {bot.wish[2]}")
                bot.wish = None
            if bot.wish is None:
                plan = bot.plan(w, rng)
                if plan is None:
                    log(f"epoch {epoch}: {bot.strategy}: nothing to wish for")
                    continue
                args, what = plan
                code, out = spark(*args, "--key", bot.key)
                m = re.search(r"accepted: proposal ([0-9a-f]{64})", out)
                if code != 0 or not m:
                    log(f"epoch {epoch}: {bot.strategy}: {what} refused: {out.splitlines()[-1] if out else code}")
                    continue
                bot.wish = (m.group(1), epoch, what)
                log(f"epoch {epoch}: {bot.strategy}: {what} ({m.group(1)[:8]})")
            code, out = spark("mine", bot.wish[0], "--minutes", f"{SECONDS / 60:.3f}", "--key", bot.key)
            if code != 0:
                log(f"epoch {epoch}: {bot.strategy}: mining failed: {out.splitlines()[-1] if out else code}")


if __name__ == "__main__":
    main()
