# FAQ

## The basics

**What is Protogaea?**  
A persistent digital world where organisms evolve on their own while people watch, follow lineages and occasionally make small interventions. See the [overview](overview.md).

**Is it a game?**  
It is a world to watch with a light layer of play. There is no winning. Your goals are your own: keep a clade alive, test a hypothesis, see what happens when hunters reach an island.

**Do I need an account?**  
Not to watch. To kindle sparks or create wishes you need a key, which the app creates for you. There is no registration and no email.

**When can I try it?**  
There are no dates yet. The project is at the specification stage; the [roadmap](roadmap.md) shows the order of work.

## Money and crypto

**Is this a cryptocurrency or a blockchain project?**  
No. There is no token, no coin, no NFTs and no blockchain. Protogaea borrows some verification techniques from that world (proof of work, Merkle trees, public randomness), but nothing in it has a price.

**Can I earn anything?**  
No. Sparks are not a currency: they cannot be saved up, transferred, sold or traded. They exist only as work bound to one wish.

**Can I pay to get better odds?**  
No. There is nothing to buy. Donations to the project, if we accept them, give no influence over the world.

**Why use proof of work at all?**  
Because interventions must be scarce, earned and verifiable without accounts. Proof of work lets anyone contribute, lets anyone check that the work was done, and needs no identity checks. It also fits the fantasy: you give the world your machine's time, not your money.

**Isn't proof of work a waste of energy?**  
It does use energy, and we do not pretend otherwise. Sparks are optional: watching is free. The client shows an energy estimate for your device, stops when your wish is done and pauses on battery power. The price of a miracle limits how much work the whole world can absorb.

## Fairness

**Can someone with a server farm dominate the world?**  
More computing power means more influence — that is true of every proof-of-work system, and we say so openly. What protects the world is its limits: at most three small miracles per epoch, cooldowns on regions and clades, a price that rises with demand, and public statistics showing how much of each wish came from its biggest supporters. Cooperation is the counterweight: many people can pool sparks on the same wish.

**Can I create lots of keys to get around limits?**  
You can, which is why we do not rely on per-key caps. Keys are free; work is not.

**Why can't I design my own creature?**  
Because the rules are open and the world can be simulated locally, so free design would turn into everyone submitting the same optimal build. Instead you can revive an extinct clade from the museum, with at most two small edits. Every genome in the world has a natural origin.

## Safety

**Will the spark client run in the background?**  
No. It starts only when you press start, shows that it is working and stops on its own when your wish is done, when the session limit ends, on battery power and on overheating. It never starts with your system unless you turn that on.

**Will my antivirus complain?**  
It might: some antivirus products flag any proof-of-work software. The client is open source, built reproducibly and signed, and we submit builds to antivirus vendors in advance.

**Can I use my phone?**  
To watch, yes. Kindling sparks on phones is not planned: app store rules forbid this kind of computation on the device, and it drains the battery.

## The world

**How big is the world?**  
64 × 64 cells, up to 6,000 organisms, at most four per cell.

**How fast does evolution happen?**  
The target is at least 30 generations per world day (288 epochs, about 24 hours).

**What if everything dies?**  
A spore bank can repopulate the world with the original genomes, at most once per world day. If that happens three times in a week, the season is declared over by extinction. The world is never restarted in secret.

**Can the rules change in the middle of a season?**  
No. A season's rules are published before it starts and stay fixed. If a bug forces a change, the season is halted with a public report and continues as a new season with a new ruleset.

## Trust

**How do I know the world is not faked?**  
You can recompute it. The core is deterministic and open source; the log, the rules and the snapshots are public; your browser can recompute recent epochs and compare the result with the published state. Independent watchers do the same continuously.

**What can the operator still do?**  
Refuse a spark, delay publication or stop the world. These limits are stated openly as the v0 trust boundary ([spec §21](spec/spec-v0.2.md#21-verifiability-and-the-trust-boundary)). A network of independent nodes may come later, if the project needs it.

## The project

**Why is it open source?**  
Because verification only means something when anyone can read and run the code. See [decision 0007](decisions/0007-open-source-from-first-commit.md).

**Why is it called Protogaea?**  
It means "proto-Earth", and it is also the title of Leibniz's treatise on the early Earth. Pangea is the supercontinent of Season 1, not the name of the project. See [decision 0009](decisions/0009-name-protogaea.md).

**Can I run my own world?**  
Later. Private worlds — for example, an evolution lab for a classroom where the teacher hands out sparks — are planned after the MVP.

**How can I help?**  
Read the specification and tell us what is unclear or wrong. See [CONTRIBUTING.md](../CONTRIBUTING.md).
