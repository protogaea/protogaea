#!/usr/bin/env python3
"""Parameter search for the Season 1 ruleset (stage A3): full seasons of the balance harness for
ruleset variants, ranked by the ecosystem health checks of spec §28.

Each variant is the default ruleset with one or more parameters changed; every variant runs on the
same seeds, so the results compare like with like. Finished variants are kept in the output
directory, and a rerun skips them.

  python3 harness/search.py screen  [--seeds 1..13] [--days 42] [--threads N] [--out runs/search]
      One parameter at a time, below and above its default (the grid in SCREEN).
  python3 harness/search.py combine NAME PATH=VALUE [PATH=VALUE ...] [--seeds ...] [--out ...]
      One variant with several changes, e.g. the best of the screen together.
  python3 harness/search.py report  [--out runs/search]
      The table of every finished variant, best first.

PATH is a dotted path into the ruleset JSON (`events.plague_mortality_ppm`, `weights.crowd_per_neighbor`).
Needs a release build of the harness (`cargo build --release -p protogaea-harness`).
"""
import csv
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HARNESS = ROOT / "target" / "release" / ("protogaea-harness.exe" if os.name == "nt" else "protogaea-harness")

# Parameters that shape diversity and the continents' fauna, each below and above its default.
# `clade_split_distance` is left out on purpose: it changes what counts as a clade, not the world.
SCREEN = [
    ("mutation_ppm", [70000, 140000]),
    ("habitat_modifier_pct", [5, 20]),
    ("kin_distance", [1, 3]),
    ("hunt_hunger_pct", [50, 70]),
    ("predation_efficiency_pct", [50, 70]),
    ("events.plague_min_share_permille", [200, 400]),
    ("events.plague_mortality_ppm", [350000, 650000]),
    ("weights.crowd_per_neighbor", [100, 220]),
    ("weights.dispersal_per_step", [5, 20]),
    ("weights.habitat_bonus", [120, 300]),
    ("rifts.rescue_radius", [5, 12]),
    ("shallow_drain", [100, 300]),
]


def args_of(argv):
    opts = {"seeds": "1..13", "days": "42", "threads": str(os.cpu_count() or 1), "out": "runs/search"}
    rest = []
    i = 0
    while i < len(argv):
        if argv[i].startswith("--"):
            opts[argv[i][2:]] = argv[i + 1]
            i += 2
        else:
            rest.append(argv[i])
            i += 1
    return opts, rest


def default_ruleset():
    out = subprocess.run([str(HARNESS), "ruleset"], check=True, capture_output=True, text=True).stdout
    return json.loads(out)


def set_path(rules, path, value):
    node = rules
    keys = path.split(".")
    for k in keys[:-1]:
        node = node[k]
    if keys[-1] not in node:
        sys.exit(f"no such parameter: {path}")
    node[keys[-1]] = value


def run_variant(name, changes, opts):
    out = Path(opts["out"])
    out.mkdir(parents=True, exist_ok=True)
    result = out / f"{name}.csv"
    if result.exists():
        print(f"{name}: already done")
        return
    rules = default_ruleset()
    for path, value in changes:
        set_path(rules, path, value)
    rules_file = out / f"{name}.ruleset.json"
    rules_file.write_text(json.dumps(rules, indent=1))
    (out / f"{name}.changes.json").write_text(json.dumps(changes))
    print(f"{name}: {', '.join(f'{p}={v}' for p, v in changes) or 'defaults'}", flush=True)
    tmp = out / f"{name}.csv.part"
    log = out / f"{name}.log"
    with open(log, "w") as f:
        subprocess.run(
            [str(HARNESS), "sweep", "--seeds", opts["seeds"], "--days", opts["days"], "--threads", opts["threads"],
             "--ruleset", str(rules_file), "--out", str(tmp)],
            check=True, stdout=f, stderr=subprocess.STDOUT)
    tmp.rename(result)


def score(rows):
    """Aggregates over seeds: the share passing each key check and the means behind them."""
    n = len(rows)
    num = lambda k: [float(r[k]) for r in rows if r[k] != ""]
    mean = lambda xs: sum(xs) / len(xs) if xs else float("nan")
    diverse = num("diverse_pct_after_day_3")
    fauna = [int(r["final_composition_permille"]) >= 800 and float(r["final_hue_divergence"]) >= 30 for r in rows]
    alive = [r["extinct"] == "false" and int(r["final_population"]) > 0 for r in rows]
    return {
        "seeds": n,
        "all_checks": mean([float(r["checks_passed"]) for r in rows]),
        "diverse_95": sum(p >= 95 for p in diverse) / n * 100,
        "diverse_mean": mean(diverse),
        "fauna_apart": sum(fauna) / n * 100,
        "comp_mean": mean([int(r["final_composition_permille"]) / 10 for r in rows]),
        "alive": sum(alive) / n * 100,
        "equil_mean": mean(num("equilibrium_pct")),
        "stories": mean(num("stories_per_day")) if rows and "stories_per_day" in rows[0] else float("nan"),
    }


def report(opts):
    out = Path(opts["out"])
    table = []
    for f in sorted(out.glob("*.csv")):
        rows = list(csv.DictReader(open(f)))
        changes = json.loads((out / f"{f.stem}.changes.json").read_text())
        table.append((f.stem, changes, score(rows)))
    table.sort(key=lambda t: (-t[2]["alive"], -t[2]["all_checks"], -t[2]["diverse_mean"]))
    print(f"{'variant':34} {'checks':>6} {'div95%':>6} {'div%':>6} {'fauna%':>6} {'comp%':>6} {'alive%':>6} {'equil':>6} {'st/day':>6}")
    for name, _, s in table:
        print(f"{name:34} {s['all_checks']:6.2f} {s['diverse_95']:6.0f} {s['diverse_mean']:6.1f} "
              f"{s['fauna_apart']:6.0f} {s['comp_mean']:6.1f} {s['alive']:6.0f} {s['equil_mean']:6.1f} {s['stories']:6.1f}")


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    command = sys.argv[1]
    opts, rest = args_of(sys.argv[2:])
    if command == "screen":
        run_variant("baseline", [], opts)
        for path, values in SCREEN:
            for v in values:
                run_variant(f"{path.split('.')[-1]}={v}", [(path, v)], opts)
        report(opts)
    elif command == "combine":
        name, pairs = rest[0], rest[1:]
        changes = [(p.split("=")[0], json.loads(p.split("=", 1)[1])) for p in pairs]
        run_variant(name, changes, opts)
        report(opts)
    elif command == "report":
        report(opts)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
