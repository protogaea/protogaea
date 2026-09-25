# protogaea-harness

The balance harness ([spec §28](../docs/spec/spec-v0.2.md#28-ecosystem-health-the-balance-harness)): it runs worlds offline, measures their ecosystem health and draws reports. It is not consensus code.

## Usage

```sh
# One world: progress, then runs/seed-1/{metrics.csv, summary.json, ruleset.json, report.html}
cargo run --release -p protogaea-harness -- run --seed 1 --days 3

# Many worlds in parallel: the ecosystem health checks for each seed, and pass rates
cargo run --release -p protogaea-harness -- sweep --seeds 1..17 --days 2 --out sweep.csv

# State hashes at checkpoints, for cross-platform determinism checks (used by CI)
cargo run --release -p protogaea-harness -- hash --seeds 1,2 --epochs 500 --every 100

# The default ruleset as JSON; edit it and pass it back with --ruleset
cargo run --release -p protogaea-harness -- ruleset > my-rules.json
```

`report.html` is self-contained: open it in a browser to watch the map, the population by type, the Muller plot of clade shares and the mean traits.

The beacon is replaced by a value derived from the run seed, so every run is reproducible.

## Checks

These are the stage A1 subset of spec §28. The thresholds are candidates.

- no total extinction;
- predators survive to the end;
- at least 6 clades of 20+ organisms after day 3;
- the dominant clade changes at least once per 3 days;
- no clade holds more than 60% of the population for more than 3 days;
- under 1% of ticks at the global limit;
- equilibrium population at 40–70% of the limit;
- at least 30 generations per world day.

## Findings (stage A1)

Measured on 2026-09-25 with 16 seeds.

- **The world lives.** With the default rules the population settles at about 3,000 organisms (about 50% of the limit), with more than 30 generations per world day and no ticks at the global limit.
- **Predators overexploit and die out.** With the initial rules (no cover, a dispersal bonus of 50), hunters wipe out the grazers within 2–3 epochs, then starve among armored organisms they cannot beat. They die out on 16 of 16 seeds.
- **Satiation alone does not help** (0 of 16 seeds): a fed hunter breeds and is hungry again.
- **Smaller gains from prey help little:** predators survive on at most 25% of seeds.
- **Cover works together with a small dispersal bonus.** With cover of +12 in forests, +10 in swamps and +14 in mountains, and a dispersal bonus of 10 per step instead of 50, predators survive one day on 13 of 16 seeds and three days on 14 of 16.
- **Still open:** in most seeds a single clade holds more than 60% of the population for almost the whole three days.

Satiation and cover were adopted into the specification (§11.3) on 2026-09-25. The default ruleset now includes cover and the dispersal bonus of 10.
