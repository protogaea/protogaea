# Protogaea in plain words

> This is a friendly introduction. The complete design is in the [specification](spec/spec-v0.2.md).

## A world that keeps going

Protogaea is a small world — 64 by 64 cells — that never stops. It has forests, steppes, deserts, mountains and swamps. Its organisms eat plants or each other, move around, reproduce and pass their traits on to their offspring with small mutations. Nobody scripts what happens next. Lineages rise, spread, collide and disappear.

Each organism is built from six traits that share a fixed budget: movement, perception, plant eating, hunting, defense and fertility. A fast hunter pays for its speed and its jaws with something else. Which trade-offs win depends on the biome, the time of year and the neighbors — and the answer keeps changing. Winters thin out populations, wildfires clear land that later grows back richer, and plague strikes hardest at whichever clade has become too dominant.

## What you do: watch like a naturalist

Most people will only watch, and that is the point.

- On the **map**, related organisms share a color, so you can see families spread, split and retreat.
- The **Muller plot** shows the whole season as flowing bands of clades: who rose, who replaced whom, and when.
- The **museum** remembers every named clade that went extinct.
- When you come back, the **"While you were away"** digest tells you what happened to the things you follow: *"Your ringed organism's great-grandchildren crossed the mountains. The blue clade lost the steppe to newcomers."*
- You can write down a **hypothesis** — "this clade will be gone by epoch 3000" — and the world will score it for you.

## Sometimes: a small miracle

You cannot control the world, but you can nudge it. The only way is a **wish**: a public proposal for one small intervention.

- **Weather:** rain or drought over a 7 × 7 area for three epochs.
- **Migration:** carry up to three organisms somewhere else — even across the sea.
- **Revival:** bring back an extinct clade from the museum.

Wishes are fueled by **sparks**: proof of work that your computer calculates while you choose to let it. Anyone can add sparks to anyone's wish, so people can join forces. When a wish gathers enough work, it becomes a **miracle**. The whole world gets at most three miracles per epoch.

After a miracle, you can see **what would have happened without it**. The world is fully deterministic, so the "shadow world" without your miracle runs with exactly the same luck for everything else. The difference between the two is the effect you had.

## Why computing power, and why no money

Sparks make interventions scarce and earned: a naturalist pays with their machine's time, not with money. There is no token, nothing to sell, nothing to earn and no way to buy better odds.

We are honest about the trade-offs. Proof of work uses electricity, so the client shows an energy estimate and stops on its own. Whoever has more computing power has more influence, just as in any proof-of-work system. What protects the world is not equality between people but the limits: small effects, cooldowns on regions and clades, a price that rises with demand, and public statistics showing how concentrated the influence is.

## Seasons: chapters of a geological story

Each season is one chapter of Protogaea's history, with fixed rules published before it starts.

**Season 1, The Breaking of Pangea**, lasts 42 world days. It begins with one supercontinent. Along published lines, cracks turn into faults, faults into shallows and shallows into open sea. For a while a few land bridges remain; then they go under one by one, each on an announced date. By the end, three or four continents carry their own diverging faunas. Moving organisms across the water before the last bridge falls is the season's defining challenge.

When the season ends, its survivors seed the next chapter — perhaps an ice age, an archipelago rising from the sea, or a meteor.

## Trust without a blockchain

There is one server, but you do not have to take its word for anything.

- The simulation is a deterministic program. Anyone can download the log and recompute every epoch, and your browser can check the published results itself.
- Every spark goes into a public log that is sealed before the random value for the epoch is revealed, so the operator cannot quietly add or drop sparks to steer the outcome.
- History is anchored in time through OpenTimestamps, so rewriting it later would be detectable.
- What the operator *can* still do — refuse a spark, delay publication or stop the world — is written down openly as the v0 trust boundary.

## What exists today

Only the design. The [roadmap](roadmap.md) explains the order of work: first the simulation core and the tools to balance it, then the viewer, then a closed test with invited people, and only then sparks and a public season.
