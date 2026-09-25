# Technical Specification: Protogaea — a Digital Evolution World

**Version:** 0.2, revision of 2026-09-25. Supersedes v0.1; changes are listed in §0. The working name changed from "Pangea" to "Protogaea" (§26); Pangea is now the supercontinent of Season 1.  
**Language:** this English version is canonical. A Russian translation is maintained in [`translations/ru/spec-v0.2.ru.md`](translations/ru/spec-v0.2.ru.md); if the two differ, this version prevails.  
**Repository:** [github.com/protogaea/protogaea](https://github.com/protogaea/protogaea), public since 2026-09-24.  
**Status:** design specification for the first public prototype and Season 1, "The Breaking of Pangea". Numbers marked "candidate" are tuned with the balance harness (§28) and frozen before the season starts.  
**Structure:** five parts, each of which can become a separate document. Part I — product and game design; II — world rules (`ruleset` v0); III — the spark protocol; IV — architecture, operations and openness; V — acceptance and stages. Terms are collected in Appendix A; comparable projects are reviewed in Appendix E.

## 0. What changed since v0.1

| Area | v0.1 | v0.2 | Why |
|---|---|---|---|
| Story | A continuous world with no arc | Season 1 "The Breaking of Pangea": the supercontinent splits on a published schedule (§4) | Time gets a direction, the season gets a beginning and an end, and viewers get events worth showing up for live |
| Ecology | Static environment; hunting unspecified | Yearly cycle, natural events, hunting with cyclic dominance, decomposition, behavioral genes, a neutral color gene (§10–11) | In a static environment evolution quickly finds an optimum and freezes |
| Hunting balance | — | Satiation and cover (§11.3), added after the first balance-harness runs in stage A1 | Without them, predators wiped out their prey within a few epochs and then died out on 16 of 16 seeds |
| Allocating influence | Lottery: 3 slots per epoch | Wishes accumulate sparks across epochs; a miracle executes once its price is reached; the price has a floor, and there are at most 3 miracles per epoch (§6, 19) | The lottery frustrates a large audience and is worthless for a small one; cooperation becomes possible |
| Actions | `found_lineage` with an arbitrary genome; `weather` 3×3 for 12 ticks | `revive` from the museum or spore bank with at most 2 edit steps; `weather` 7×7 for 36 ticks with a cooldown; free `ring` (§5) | No "meta-build" optimization; weather effects are noticeable; researchers get an action that needs no sparks |
| Randomness | Stateful generator | Counter-based randomness `H(seed, tick, subject, purpose, k)` (§14) | Independence from call order, safe parallelism, honest counterfactuals |
| Determinism for players | Audit only | "What if", ensemble forecasts, the core in WASM, root verification in the browser (§7, 22) | The causal chain is visible; every viewer can check the trust claims |
| Observation | Map, feed, cards, time machine | Plus Muller plot, clade tree, automatic names, "While you were away", subscriptions, story detectors, museum, chronicle, hypothesis journal, live stream (§7) | Coming back is the main product hypothesis; now it has mechanisms |
| Protocol | Receipts, but no commitment to the ticket set before the beacon | A spark log modeled on Certificate Transparency and committed before the beacon round, cheap-DoS protection, `proposal_id` excluding the signature, `pers` in yespower, OpenTimestamps, an epoch timeline (§16–21) | The operator can no longer steer outcomes by selective inclusion; deadlines are unambiguous |
| State | Includes lineage history | Only what affects the future; `state_root` is a Merkle root (§15) | Bounded size; provable existence of an organism |
| Platforms | Win/Linux x86-64, Linux/Android ARM64 | Plus macOS arm64 and Raspberry Pi; Android out of the MVP; a single desktop app; a WASM spark client in the browser (§24) | The real ARM64 audience, a no-install entry point, one place for keys |
| World vocabulary | "Mining", "tickets" | In the UI: sparks, wishes, miracles (§2) | Antivirus software, app store rules, crypto associations |
| Stages | A → B → C → D → E | Plus B′, a closed observation test without PoW; the balance harness in A; metrics declared in advance (§30) | The riskiest hypothesis is tested before the most expensive part is built |
| Openness and project language | Undefined | Code open from the first commit, loud announcement after stage B; Apache-2.0 for everything needed for verification, AGPL-3.0 for the service; English for code and documentation, UI in English and Russian from day one (§26) | The trust model is impossible without open code; the English-speaking audience is larger |
| Name | "Pangea" | The world and product are called "Protogaea" ("proto-Earth"); Pangea is the supercontinent of Season 1 (§4, 26) | "Pangea" is a high-risk brand: games on Steam, EU trademarks for video games, crypto projects, taken domains. "Protogaea" covers all future seasons |

# Part I. Product and game design

## 1. Idea, player fantasy and the limits of our claims

There is a continuous digital world — Protogaea, the "proto-Earth". Organisms forage, move, hunt, reproduce, pass on traits with mutations, and go extinct. The environment changes too: the times of year turn, wildfires and plagues happen, and in the first season the continent itself breaks apart. The player watches the map, the family trees and the history, and **kindles sparks on their own processor** — computational work that grants a limited right to intervene: change the weather in a region, relocate a few organisms, or revive an extinct branch. After an intervention the ecosystem develops on its own; the outcome is neither promised nor chosen by the player.

**The player fantasy: a naturalist with a small budget of miracles.** The player looks at the world like a scientist: follows branches, forms hypotheses, and compares forecasts with what actually happened. They intervene rarely and pay with their machine's time, not with money. The tone of the interface, the names and the sound all serve this fantasy: a calm naturalist's journal, not a trading terminal.

| Layer | Role | What we do not claim |
|---|---|---|
| Sparks (PoW) | Limit and allocate the ability to intervene; make the work spent verifiable. | That computing sparks "computes evolution" or proves that a CPU in particular was used. |
| Simulator | Computes the world state uniformly from the initial state and the event log. | That a beautiful picture proves the computation is correct. |
| Evolution | Inheritance, random variation, differential survival and reproduction. | That the world will keep growing more complex or become a scientific model of Earth's ecology. |

**Main product hypothesis:** people want to follow the fate of branches, discuss unexpected events, and come back to influence the ecosystem with care. Papers on Avida name three necessary properties of digital evolution — replication, heritable variation and differences in fitness; Tierra already treated CPU time as a resource and memory as an environment. These are design sources, not grounds for claiming scientific novelty (Appendix D).

**The project's key criterion:** take away the sparks, leave only the ecosystem's stories — and people still want to open the world tomorrow. Sparks give participants an understandable, bounded influence on those stories.

Principles shared by all seasons:
- Reading requires nothing: no registration and no sparks.
- Nobody can intervene in the world without sparks. The only exception is ringing (`ring`), which does not affect the world.
- No token, no promise of earnings, no NFTs, no sale of influence, no paid odds. Donations to the project give no influence over the world.
- Season rules are never changed quietly, and the world is never restarted in secret.
- Hidden computation on a device without its owner's explicit consent is forbidden.

## 2. The language of the world

Internal documents and code use technical terms (PoW, share, proposal). The interface speaks the language of the world. The word "mining" (Russian «майнинг») is not used in the interface, in program names or in app store listings: it triggers antivirus software, conflicts with app store rules, and evokes crypto for exactly the audience that was promised "no token".

| Technical term | UI (EN, candidate) | UI (RU) | Meaning |
|---|---|---|---|
| PoW computation | kindle sparks | разжигать искры | Processor work on a proof of work for a specific wish. |
| Share (`spark`) | spark | искра | A proof of work accepted by the server; it carries a weight in work units. |
| Proposal (`proposal`) | wish | замысел | A public proposal for an intervention; accumulates sparks from any naturalist. |
| Executed intervention (`miracle`) | miracle | чудо | A wish that has reached its price and been applied to the world. |
| Price (`price`) | price of a miracle | цена чуда | The threshold of accumulated work after which a wish is ready to execute. |
| Key holder | naturalist | натуралист | A profile bound to a public key. |

English is the default interface language; Russian is the second language from day one (§26). The English terms are candidates: native speakers review them before stage B. The fallback for "wish" is "intent". In this document, "wish" is the product term; code and protocol fields use `proposal`.

## 3. Audience, roles and the game loop

- **Viewers** watch the map, the Muller plot, the feed and the chronicle without registering or kindling sparks. When they come back, they get a "While you were away" digest built from data stored in their browser.
- **Researchers** ring organisms, subscribe to branches and regions, keep a hypothesis journal, study counterfactuals and publish links to stories. They need no sparks.
- **Naturalists who kindle sparks** create wishes or support other people's wishes, kindle sparks on their own devices, and follow the progress and the result.
- **Private world creators (after the MVP)** run their own world with published rules, a separate log and their own source of sparks. The main candidate is education: an evolution lab for a classroom, where the teacher hands out sparks instead of PoW.

**The game loop:** notice → forecast → propose → kindle → miracle → observe → compare → share → return.

1. Notice a problem or an opportunity: in the feed, through a subscription, or in the "While you were away" digest.
2. Run an ensemble forecast: how the region is likely to behave with and without an intervention.
3. Create a wish with a hypothesis ("relocated hunters will halve the island's herbivores within 36 epochs") or support someone else's.
4. Kindle sparks and watch the wish approach its price, see who else supports it and where other people's attention is going.
5. The miracle executes and is marked on the map and on the Muller plot.
6. Observe the consequences over generations.
7. Compare the outcome with the counterfactual "what would have happened without the miracle" and with your own hypothesis; the hypothesis is scored automatically.
8. Share the story through a permanent link with a preview.
9. Come back through a notification or the digest.

The interface shows the causal chain ("three organisms were moved across the strait; seven generations later the branch held 18 cells; without the miracle the island would have stayed empty until epoch 1234"), not a hashrate.

**Social layer.** Wishes are public, so naturalists rally around shared goals ("save the blue branch") or pull the world in different directions ("rain versus drought" over the same region). The attention heat map shows where sparks are going right now. The MVP has no free-form comments: discussion happens in external communities, and each wish carries a structured hypothesis and a permanent link.

## 4. Season 1: "The Breaking of Pangea"

Each season of Protogaea is a chapter of its geological history. Chapter one is the supercontinent Pangea: Season 1 begins with a single supercontinent and ends with several isolated continents, each with its own fauna. The schedule of geological change is published in the `ruleset` before the start and derived from `genesis_seed`; there are no secret changes. Naturalists know in advance when each land bridge will close and can prepare.

**Duration (candidate):** 42 world days = 12,096 epochs. World time is logical (§20): if the server is down, the season grows longer in wall-clock hours but not in epochs.

| Phase | World days | What happens |
|---|---|---|
| I. Unity | 0–6 | One continent in the ocean. The lines of future rifts are published and shown on the map as thin cracks, but they do not affect the rules yet. |
| II. Cracks | 7–13 | Rift cells become "fault" cells: food growth ×0.5, step cost ×2. |
| III. Shallows | 14–23 | The rifts turn into shallows in waves (§10): passable, but expensive and without food. Populations begin to diverge. |
| IV. Straits | 24–34 | The shallows deepen into impassable water. 3–5 land bridges remain. |
| V. The last bridges | 35–38 | The land bridges go under one by one. Each closing is an announced event, with a reminder one day ahead. |
| VI. Continents | 39–42 | 3–4 isolated continents. Season finale, hall of fame, archive. |

**Map generator requirements:** every future continent contains all five land biomes and at least 40 starting organisms; the phase V land bridges run through different biomes. The season seed is chosen from at least 20 candidates by published criteria (the metrics of §28). All candidates and the harness results are published together with the chosen seed.

**The players' role in the story.** The main tool is `migrate`: get organisms across a strait in time, or "seed" a continent that lacks predators. `revive` brings branches that died out on one continent back to a place where they can survive. Isolation creates natural experiments: the same ancestral branch diverges differently on different continents, which shows both on the Muller plot and in the colors on the map.

**Finale and hall of fame.** At the end of the season we publish the longest-lived clade, the most widespread clade, the biggest comeback, the most supported miracle, the most accurate forecast and the strongest divergence between continents. The season archive (log, snapshots, museum) remains available permanently.

**Continuity between seasons.** The museum is shared by all seasons. The genomes of clades that survive to the finale are added to the next season's spore bank. Candidate future chapters: "Ice Age" (ice advances on a schedule), "Archipelago" (volcanic islands rise from the ocean), "Meteor" (a catastrophe announced in advance, and the recovery that follows).

## 5. Naturalist actions

| Action | Parameters | Hard check at application | Effect and limits |
|---|---|---|---|
| `weather` | Center; type `rain` or `drought` | The 7×7 area does not overlap an active effect; the region is not on cooldown (12 epochs after the previous effect ends) | 36 ticks (3 epochs): food growth ×1.5 or ×0.5, moisture ±30; never kills organisms directly |
| `migrate` | Clade, 5×5 source area, target cell | At least 10 organisms of the clade in the source area; the target is a land cell with free space nearby; the clade has not been relocated in the last 12 epochs | Moves up to 3 organisms; the source area loses them (a move, not a copy); crossing water is allowed |
| `revive` | A museum or spore bank entry; a genome edit of at most 2 mutation steps; a starting cell; a name from the root dictionary | A museum entry must have been extinct for at least 36 epochs; at most 8 organisms in the 5×5 area around the cell; the same entry can be revived at most once per world day | 5 organisms in the 3×3 neighborhood; the new clade is a child of the museum entry |
| `ring` (no sparks) | `organism_id` | The organism is alive; at most 20 rings per key per day | Only a marker and a subscription in the interface; does not affect the world and is not part of the state |

**Why `revive` instead of founding from scratch.** The rules are open and anyone can run the simulation locally, so a free choice of genome quickly turns into min-maxing optimal builds that crowd out everything evolution has produced. Revival keeps the origin of every genome natural (it arose in the world at some point), gives the museum a role in play, and creates stories ("the ancients have returned"). At the start of the season the spore bank holds the genesis genomes, so `revive` is available from day one.

**Why `weather` is bigger.** A 3×3 effect for 12 ticks is lost in natural fluctuations, and an intervention nobody can notice creates no story. Rarer interventions with visible consequences are better.

**Order of application** at the epoch boundary: `weather` → `migrate` → `revive`. If two miracles selected in the same epoch conflict (overlapping weather areas, the same clade relocated twice, occupied target cells), the lower-priority miracle is **deferred** to the next epoch and keeps its accumulated sparks.

**Invalidation.** Wishes get a soft check every time the intake window opens, against the same state the miracle would be applied to (§20). If a wish has become invalid (for example, fewer than 10 organisms of the clade remain in the source area), it is closed with status `invalidated` before anyone spends new sparks on it. Work already invested is burned, and supporters see a clear reason. The client stops kindling sparks for closed wishes on its own.

## 6. Wishes and sparks: how influence is allocated

The v0.1 lottery is replaced by work that accumulates on public wishes. This section describes the game logic; the protocol is in §17–19.

- **A wish** is created by its author: an action, parameters, a lifetime (at most 288 epochs, i.e. one world day), and optionally a hypothesis and a name. A wish is created together with its first spark, which protects against spam. A single key can have at most 3 open wishes at a time.
- **Sparks** for a wish can be kindled by any naturalist. Every spark is bound to a wish and to the key of whoever produced it; nobody can reassign someone else's work. There is no spark balance: sparks cannot be saved up, transferred, sold or traded.
- **Work accumulates** on a wish from epoch to epoch. When the accumulated work reaches the **price of a miracle**, the wish becomes ready and joins the execution queue.
- **At most 3 miracles per epoch.** Ready wishes execute in descending order of the share of their price they have covered; ties are broken by the beacon value. Wishes that do not fit wait for the next epoch and keep their sparks.
- **Bigger interventions cost more.** A wish's price is the base price multiplied by its action's coefficient (§19). Candidates: `weather` ×1, `migrate` ×1.2, `revive` ×2; to be refined in stage B′.
- **The price** behaves like difficulty: it rises when more wishes are ready than there are slots, falls when there are more slots, and never drops below the **floor**. The floor (candidate: about 8 core-hours of the reference core) ensures that a miracle costs noticeable work even with only five players.
- **Expiry.** A wish that does not reach its price within its lifetime is closed as `expired`, and the invested work is burned. Burned work appears in the statistics as "unfulfilled wishes".
- **Credit.** A miracle is recorded in history together with everyone who supported it and each supporter's share. Profiles say "took part in N miracles", not "won".

| | v0.1 lottery | v0.2 wishes |
|---|---|---|
| 1,000 equal naturalists | ≈0.9% chance per 15 minutes; ~28 h to a win on average | Visible progress; by joining forces, 100 people can fulfill a wish within a few epochs |
| 10 naturalists | Each wins roughly every 17 minutes | The price floor: a miracle requires noticeable work |
| Cooperation | Impossible | The core of the mechanic |
| Losers' work | Lost | Accumulates until the wish expires |
| Connection speed | The window has to be protected from races | Irrelevant: only the total work counts |

**An honest limitation.** Whoever has half of the computing power still has about half of the influence — just as with the lottery. Wishes change not fairness but the spread of outcomes and the character of the game. What protects the world from abrupt changes is the action limits and cooldowns (§5), not equalizing people.

**Concentration statistics.** Computing power can be rented or bought, so the concentration of influence is shown openly. Each wish card shows the share of work contributed by the top 1, 3 and 10 keys. The season statistics show the same shares across all executed miracles. Capping the share of a single key is pointless: keys are free, and such a cap is bypassed by creating new ones.

**The attention heat map.** An overlay on the map shows which regions and clades sparks are flowing to right now: the total work over recent epochs, not tied to keys. Players' work becomes visible without affecting the simulation.

## 7. Observation and stories

This is the product's main layer: by the key criterion, the world must be interesting even without sparks.

**The map.**
- Layers: biomes, food, population, weather and natural events, rifts and their schedule, clade colors (the neutral gene, §11), the attention heat map.
- Zoomed into a region, organisms are drawn as procedural glyphs derived from the genome: legs for movement, eyes for perception, jaws for hunting, a shell for defense, a belly for plant eating, a brood for fertility.
- **Live mode** replays the most recently computed epoch in real time, about 25 s per tick. It is a broadcast delayed by one epoch: a replay of published data, not a forecast. A "jump to latest" button shows the end of the latest epoch immediately.

**The Muller plot** shows clade shares over time for the whole season and is the second main tab after the map. Miracles and natural events are marked on the time axis; clicking a band opens the clade card.

**The clade tree** is a phylogeny with names and times of appearance and extinction. Founder lineages are only its top level.

**Cards.**
- Organism: genome and behavioral genes, energy, age, parent, offspring, clade, cause of death, rings.
- Clade: name, reference genome, population, range, generations, ancestors and child clades, history, related miracles.
- Miracle: the wish, its supporters, the hypothesis, the consequences, the counterfactual.

**The time machine.** Any past epoch can be opened, and any two states can be compared. Intermediate ticks are recomputed in the browser by the WASM core from the nearest snapshot. Every epoch, organism, clade, event and miracle has a permanent link; an organism's existence in an epoch is backed by an inclusion proof (§15).

**"What if".** For every miracle the server computes a shadow world without it for 36 epochs ahead and publishes the divergence: populations of the affected clades, ranges, extinctions. Counter-based randomness keeps the comparison honest: every other organism gets the same "luck". The computation can be repeated in the browser.

**Ensemble forecast.** For any wish the browser runs 32 possible futures of 12 epochs with different seeds, with and without the miracle, and shows the distribution of outcomes. The forecast is labeled honestly: "given the current state, ignoring other people's miracles and future events". Experienced players would write such tools themselves anyway, so everyone gets one.

**Feed and story detectors.** In addition to the v0.1 events (foundings, migrations, population growth, disappearance, a change in the dominant trait), detectors look for stories. Detectors run outside consensus.

| Story | Condition (candidate) |
|---|---|
| Comeback | A clade grew from ≤ 3 to ≥ 50 organisms |
| Crossing | A clade established itself beyond mountains or a strait for the first time: ≥ 10 organisms for more than 12 epochs |
| Invasion | A newcomer clade replaced a region's dominant clade |
| Arms race | In a predator–prey pair, mean hunting and defense both grew by ≥ 2 within 30 generations |
| Last of its kind | A single organism remains of a clade that once had ≥ 100 |
| Changing of the guard | The world's most numerous clade changed |
| Records | Oldest organism, longest lineage of the season |

Miracles and natural events are marked with different icons, as in v0.1.

**"While you were away".** A personal digest on return: what happened to the clades, regions, rings and wishes the user subscribes to, most significant first. For anonymous viewers the digest starts from the time of their previous visit, stored in the browser.

**Subscriptions and notifications.** Users can subscribe to a clade, a region, a wish or a ringed organism. Channels: web push and a Telegram bot, with a rate limit and a digest mode.

**The paleontological museum** keeps every extinct named clade from every season: peak population, lifespan, range map, cause of decline. A `revive` wish can be created straight from a museum card.

**The daily chronicle** is assembled from structured events using templates. An optional language-model layer can rewrite it as flowing prose; every statement in that text links to an event ID, the text is labeled as generated, and it is not part of consensus.

**Sharing and streaming.**
- Permanent links with previews: a map snapshot and a caption.
- GIFs and short videos of events, rendered from the replay.
- An embeddable widget.
- A 24/7 stream with an automatic camera that picks the most eventful region by detector scores and captions it with lines from the feed.

Priority: the "While you were away" digest and the chronicle matter more than the stream. For TechnoSphere, the closest historical analog, the main connection with more than 100,000 users was email about significant events in the lives of their creatures, while a 24/7 broadcast is technically fragile (Appendix E). That is why the digest and subscriptions belong to stage B and the stream only to stage D.

**The hypothesis journal.** Template-based predictions are checked automatically against the log:
- the population of clade X in region R by epoch E will be above or below N;
- clade X will or will not go extinct before epoch E;
- the mean of trait T in clade X will rise or fall by at least k by epoch E;
- clade X will establish itself on continent Y by epoch E.

A hypothesis attached to a wish is part of the wish's bytes. A standalone prediction is signed with the user's key, and its hash goes into the hourly OpenTimestamps anchor (§15), so a prediction cannot be backdated. The reward is reputation based on forecast accuracy (for example, a Brier score) — no money and no influence on the world.

**Names.** Each clade gets an automatic binomial name derived from its `clade_id`, dominant trait and home biome: "Cursor deserti", "Venator silvae". The author of a `revive` miracle picks a name from a dictionary of Latin roots. Genus and epithet are set by index, there is no free text, so there is nothing to moderate. The dictionary is screened in advance for undesirable combinations.

## 8. The spark interface

- **Consent before starting:** a clear warning about CPU load, an energy estimate (Wh per session on this device), a statement that there is no income, and one-click stop.
- **Choosing a wish** — one's own or someone else's. The action, its maximum effect, progress toward the price, the number of supporters, the ensemble forecast, the lifetime and the window closing time are all visible.
- **While working:**
  - sparks per minute and accepted sparks;
  - contribution to the wish, in percent;
  - time until the window closes;
  - temperature — **only if the OS exposes it**;
  - rejection reasons;
  - "gentle", "normal" and "maximum" profiles.
- **Automatic stop:**
  - when the wish has executed, expired or become invalid;
  - when the session limit ends (2 hours by default), unless the user has explicitly enabled continuous mode;
  - pause on battery power and on overheating;
  - launch at system startup is off by default.
- **Afterwards:** the wish's status and a link to the miracle and its counterfactual, or a clear explanation (`expired`, `invalidated`, deferred because of a conflict). Spark receipts are available for verification.

# Part II. World rules (`ruleset` v0)

Everything in this part is the content of `ruleset.json` and the rules specification. Once a season has started, the rules version does not change. Any change means a new season and a new `ruleset_id`, so that replays remain reproducible. The server, the independent replay and the client all use the same rules version. The client never computes an authoritative result from hidden constants.

## 9. Map, time and capacity

| Parameter | Value (candidate) | Reason or limit |
|---|---|---|
| Map | 64 × 64 cells, no wrap-around | The whole world is visible at once; regions are easy to compare |
| Land biomes | Forest, steppe, desert, mountains, swamp | Different food growth, step cost and seasonality |
| Water cells | Deep water, shallows | Deep water is impassable; shallows are passable, but expensive and without food |
| Tick | Unit of simulation | All rules in §11–13 are applied tick by tick |
| Epoch | 12 ticks, nominally 5 minutes | World time is logical (§20) |
| World day | 288 epochs = 3,456 ticks | — |
| World year | 372 epochs = 4,464 ticks (31 hours) | Not a multiple of a day: people who visit at the same hour see different times of year |
| Season 1 | 12,096 epochs (42 world days) | §4 |
| Capacity | Up to 6,000 organisms in total and up to 4 per cell | A safety net: population must be limited by food (§28) |
| Start | 300 organisms: 6 founder lineages of 50 each, different archetypes, in different biomes | The ecology develops even without players |
| Miracles | At most 3 per epoch | Player influence stays bounded relative to the autonomous ecology |

The map is generated from the published `genesis_seed`. A cell stores its biome, food stock, moisture, detritus, active effects (weather, natural events) and its rift phase if it lies on a rift line. All values are integers. Operations, rounding order and limits are defined by the rules (§14).

## 10. Times of year, climate, rifts and natural events

**Times of year.** A year is divided into four equal times of year of 93 epochs (1,116 ticks) each. Food growth in a cell is computed as:

`regen = base_regen[biome] × season_mult[biome][season] / 100 × moisture_mult(moisture) / 100 + decomposition`

Here `season` means the time of year, not a game season. The food stock never exceeds `food_max[biome]`. Candidate seasonal multipliers, %:

| Biome | Spring | Summer | Autumn | Winter |
|---|---:|---:|---:|---:|
| Forest | 110 | 120 | 90 | 40 |
| Steppe | 120 | 90 | 80 | 30 |
| Desert | 80 | 40 | 70 | 60 |
| Mountains | 70 | 100 | 60 | 20 |
| Swamp | 120 | 110 | 100 | 50 |

Winter is the main filter: it produces population booms and busts and migrations toward milder biomes. Transitions between times of year are smoothed tick by tick, with no steps.

**Moisture** ranges from 0 to 100. Every tick it returns toward the biome's base level by `moisture_relax`. The `rain` and `drought` miracles shift it by ±30. The function `moisture_mult` is defined by a table; the candidate ranges from 50% at zero moisture to 110% at full moisture.

**Rifts.** The rift lines and the phase schedule (§4) are generated from `genesis_seed` and are part of the `ruleset`. A cell changes phase at the epoch boundary given in the schedule.

| Cell phase | Food growth | Step cost | Passable |
|---|---|---|---|
| Crack (phase I) | As the biome | As the biome | Yes |
| Fault | ×0.5 | ×2 | Yes |
| Shallows | 0 | ×3; an organism that ends a tick in the shallows loses an extra `shallow_drain` energy | Yes |
| Deep water | 0 | — | No; organisms can only be carried across it by a `migrate` miracle |

Organisms standing in a cell when it turns into deep water are moved to the nearest free land by a deterministic rule. If there is no room, they die, and the log records the cause as "drowned".

**Natural events** are drawn at the epoch boundary from the epoch seed. Probabilities are integers in parts per million per epoch, published in the rules. At most two events of each type are active at once.

| Event | Where and when | Effect | Frequency (candidate) |
|---|---|---|---|
| Wildfire | Forest and steppe, summer, moisture < 30 | Within a radius of 2–4, food and detritus drop to zero and organisms lose 50% of their energy; then "ash": growth +50% for 72 ticks | Once every 2–3 world days |
| Flood | Swamps and cells next to water, spring | Cells become shallows for 24 ticks | Once a day in spring |
| Great drought | Steppe and desert, summer | A 9×9 area: growth ×0.5 for 72 ticks | Once every 3 days |
| Plague | A region where one clade is denser than a threshold | Mortality `plague_p` for that clade's organisms within radius 3; the event's probability grows with the clade's share of the world | Depends on dominance |

Plague follows the "kill the winner" principle: the more a single clade dominates, the more likely disease is to strike it. This frequency-dependent selection sustains diversity and keeps the map from turning into a monoculture. Among natural events, only plague kills directly; apart from that, organisms can drown when a rift deepens. Wildfire and drought act through energy and food.

## 11. Organisms

### 11.1 Genome

Six trait "stats" share a common budget: their sum is always 24 and each trait lies in the range 0–8, so one trait can only grow at another's expense. There are about 32,700 possible combinations, and any of them can be reached in at most 24 mutations. That is too little for open-ended evolution, so diversity comes from a changing environment, interactions between species, and behavioral genes.

| Gene | Range | Phenotype (candidate) |
|---|---|---|
| `M` movement | 0–8 | Steps per tick: `(M + 2) / 3`, i.e. 0, 1, 1, 1, 2, 2, 2, 3, 3. With `M = 0` the organism is sessile |
| `P` perception | 0–8 | Sight radius `1 + P / 3` (1–3 cells); helps spot predators and prey |
| `G` plant eating | 0–8 | Bite: `G × bite_per_point` food units per tick |
| `H` hunting | 0–8 | Attack strength; with `H = 0` the organism does not hunt |
| `D` defense | 0–8 | Defense strength |
| `F` fertility | 0–8 | Lower reproduction threshold, but weaker offspring |

Behavioral genes are outside the budget: they are strategies, not strength.

| Gene | Range | Role |
|---|---|---|
| `habitat` | 0–5 | Values 0–4 are a preferred land biome. In it metabolism is 10% lower, in other biomes 10% higher, and the organism is drawn toward it when choosing a move. Value 5 is a generalist, with no bonuses or penalties |
| `dispersal` | 0–3 | Tendency to leave settled cells and relatives even when food is sufficient |
| `boldness` | 0–3 | How much food outweighs the risk of being near a predator |
| `hue` | 0–359 | A neutral color gene: affects nothing and drifts in small steps. Relatives share a shade, so the divergence of branches is visible on the map |

Each organism stores:
- `organism_id` (u64, in order of birth);
- `parent_id`;
- `lineage_id` — the founder lineage;
- `clade_id`;
- cell, age, energy, genome.

Energy is stored as an integer in hundredths of a unit.

**Mutations** at birth happen independently of each other.
- With probability `mutation_rate`, one step is taken across the traits. A pair `(i, j)`, `i ≠ j`, is chosen uniformly among the **valid** pairs (`g_i < 8`, `g_j > 0`), then `g_i += 1`, `g_j −= 1`. There are no retries.
- With probability `behavior_mutation_rate`, one behavioral gene changes: `habitat` gets a random different value, `dispersal` and `boldness` shift by ±1 within their range.
- With probability `hue_mutation_rate`, `hue` shifts by ±1…8 around the color wheel.

Without a mutation the genome is copied exactly.

### 11.2 Energy

Cost per tick:

`cost = base_metabolism + Σ trait_upkeep[k] × g_k + habitat_modifier + move_cost[biome] × steps + attack_cost × attacks`

Traits have different upkeep: hunting, defense and mobility cost more than plant eating and fertility. The energy store never exceeds `energy_max`, so an organism cannot hoard energy indefinitely.

**Eating plants.** If the organism did not attack this tick and has `G ≥ 1`, it eats `min(food[cell], G × bite_per_point)` food units and gains the amount eaten `× plant_efficiency / 100` as energy. The tick's queue decides who eats first in a shared cell.

### 11.3 Hunting

An organism with `H ≥ 1` can attack an organism it has spotted in its own or a neighboring cell if their genomes differ by more than `kin_distance` (candidate: 2) — "kin do not eat kin".

`attack = H × attack_weight + P + rand(0 … roll_span)`  
`defense = D × defense_weight + M + P_prey / 2 + cover + rand(0 … roll_span)`

If `attack > defense`, the prey dies (cause: predation). The hunter gains `prey_energy × predation_efficiency / 100 + body_value`, and the remains become detritus. On failure the hunter loses only `attack_cost`. At most one attack per tick.

**Satiation.** An organism hunts, and looks for prey when choosing a move, only while its energy is below `hunt_hunger_pct` of `energy_max` (candidate: 60%). A fed predator does not kill.

**Cover.** An organism standing in a biome with cover adds that biome's `cover` to its defense (candidates: forest +12, swamp +10, mountains +14; open land 0). Prey seek cover when a predator is near, and hunters do best in open land. Both rules were added in stage A1: without them, predators wiped out their prey within a few epochs and then died out on 16 of 16 seeds; with them, predators survived three world days on 14 of 16 seeds ([harness findings](../../harness/README.md#findings-stage-a1)).

**Intended cyclic dominance** (checked by the "archetype arena", §28):
- plant eaters (high `G`) beat armored organisms (high `D`) in the competition for food;
- armored organisms beat hunters: hunters cannot eat them and starve nearby;
- hunters beat plant eaters.

On a grid, such a triad with no overall winner produces traveling waves and spirals — the well-known spatial rock–paper–scissors effect (Appendix D). In predator–prey pairs it produces population oscillations and arms races.

### 11.4 Choosing a move

Each organism with `steps ≥ 1` scores the cells within its sight radius and picks the target with the highest score. It then moves toward the target by at most `steps` cells along a deterministic path: each step reduces the Chebyshev distance, and the order in which directions are tried is fixed.

`score(c) = w_food × expected_food(c) + w_hunt × prey_vulnerability(c) − w_danger × threat(c) × (4 − boldness) + w_habitat × [biome(c) = habitat] − w_crowd × occupancy(c) − w_move × path_cost(c) + w_disp × dispersal × distance_from_kin(c)`

All weights are integers from the `ruleset`. Cells that already hold 4 organisms are unavailable. Ties are broken by counter-based randomness (§14). An organism with `M = 0` does not move, but it still eats and defends itself.

### 11.5 Reproduction

If an organism's energy is at least `repro_threshold(F) = repro_base − F × repro_per_F`, it produces one offspring per tick.
- Offspring energy: `child_energy(F) = child_base − F × child_per_F`. The parent gives this energy and additionally pays `birth_cost`. Fertile organisms reproduce more often but produce weaker offspring — a trade-off between the quantity and quality of offspring.
- The offspring appears in the parent's cell if it holds fewer than 4 organisms; otherwise in a free passable neighboring cell chosen by counter-based randomness.
- If there is no room or the global limit of 6,000 organisms has been reached, no birth happens and no energy is spent. Such events are counted: the share of ticks spent at the limit is one of the metrics in §28.

### 11.6 Death and decomposition

Causes of death: starvation (energy ≤ 0), old age, predation, plague, drowning. After age `senescence_start` (candidate: 150 ticks) the probability of dying in a tick rises linearly and reaches 100% at `max_age` (candidate: 300 ticks). The cause of death is recorded.

If an organism did not die to a predator, its body (`body_value` plus part of its remaining energy) becomes detritus in its cell. Every tick, `decomposition_rate`% of the detritus turns into food growth. Graveyards become gardens, and a feedback loop appears between death and fertility.

### 11.7 Parameter table

All energy costs and gains are gathered in a single complete table in `ruleset.json`.

| Group | Parameters |
|---|---|
| Environment and food | `base_regen`, `food_max`, `season_mult`, `moisture_mult`, `moisture_relax` |
| Movement | `move_cost`, `shallow_drain` |
| Metabolism | `base_metabolism`, `trait_upkeep`, `habitat_bonus`, `energy_max` |
| Feeding | `bite_per_point`, `plant_efficiency` |
| Hunting | `attack_weight`, `defense_weight`, `roll_span`, `attack_cost`, `predation_efficiency`, `body_value`, `kin_distance`, `hunt_hunger_pct`, `cover` (per biome) |
| Reproduction | `repro_base`, `repro_per_F`, `child_base`, `child_per_F`, `birth_cost` |
| Aging and decomposition | `senescence_start`, `max_age`, `detritus_share`, `decomposition_rate` |
| Mutation | `mutation_rate`, `behavior_mutation_rate`, `hue_mutation_rate` |
| Other | `w_*` weights, natural event probabilities, `plague_p`, clade parameters (§12) |

The numbers are tuned with the balance harness (§28). The goal is for the equilibrium population that food can sustain to be 40–70% of the global limit.

## 12. Clades, the museum and the spore bank

**Clades.** The number of founder lineages only goes down over time: all living organisms gradually turn out to descend from a few ancestors. That is why the main unit of the interface is the clade — a branch of the family tree.
- A clade stores its reference genome, its parent clade, the epoch it appeared in and the number of living organisms.
- Offspring inherit the parent's `clade_id`. If an offspring's genome has moved `clade_split_distance` steps or more from the clade's reference genome (candidate: 3), the offspring founds a child clade with its own genome as the reference.
- A clade gets a name (§7) once at least `clade_name_threshold` organisms (candidate: 20) are alive in it at the same time. A "new species" in the interface is a conventional name for a branch, not a claim of biological speciation.
- The distance between genomes is half the sum of the absolute differences of the traits, i.e. the number of mutation steps. Behavioral genes and `hue` do not count toward the distance.

**Clade extinction.** When a clade has no living organisms left, a `clade_extinct` event with the clade's full record is written to the log, and the clade is removed from the state. A named clade goes into the museum.

**The museum in the state.** So that the core can validate `revive` without external data, the state holds a bounded museum: the 1,024 most recently extinct named clades with their reference genomes. Older entries remain in the log and in the public museum, but they cannot be revived.

**The spore bank** holds the genesis genomes. From the second season on, the genomes of clades that survived to the previous season's finale are added to it.
- **Natural revival.** If fewer than 30 organisms are alive, then at the epoch boundary the spore bank places 5 organisms of each genesis genome into suitable cells chosen by counter-based randomness. This happens at most once every 288 epochs, and each trigger is a major event in the feed.
- **End of season.** The world is never restarted in secret. If the spore bank triggers 3 times within 7 world days, the season is declared ended by extinction.

## 13. Epoch and tick order

**Epoch boundary.** Before the ticks begin, the following runs in a fixed order:
1. Rift phase changes according to the schedule.
2. Drawing natural events.
3. Applying miracles: `weather` → `migrate` → `revive`.
4. Natural revival from the spore bank, if its condition holds.

**Tick.** Repeated 12 times per epoch:
1. Environment: time of year, moisture, food growth, decomposition of detritus, active weather and event effects.
2. Queue: a stable permutation of organisms by `H(epoch_seed, tick, organism_id)`, so that a lower ID never gives a permanent advantage.
3. Actions in queue order: perception → target choice → movement → eating or attacking. Conflicts over food and space are resolved by the queue. An organism that died before its turn does not act.
4. Energy costs; deaths from starvation, old age and plague; detritus formation.
5. Births, mutations, placement of offspring, clade updates.
6. Statistics and story detectors. This step is outside consensus: it is computed from the log and is not part of `state_root`.

## 14. Determinism and randomness

**Counter-based randomness.** The generator has no state. Every random number is computed as:

`rand(purpose, subject, k) = u64_le(BLAKE3("PROTOGAEA/RAND/V0" ‖ epoch_seed ‖ tick ‖ subject ‖ purpose ‖ k)[0..8])`

- `subject` is the ID of an organism, a cell or an event;
- `purpose` is a code for the use: queue, mutation, attack, placement, and so on;
- `k` is the attempt number.

A uniform number in `[0, n)` is obtained by rejecting unsuitable values and incrementing `k`, so there is no modulo bias.

What this gives us:
- the result does not depend on call order, so computation within a tick can be parallelized as long as conflicts are resolved deterministically;
- counterfactuals are honest: remove a miracle, and every other organism gets the same "luck";
- there is no generator state to store or hash.

**Epoch seed:** `epoch_seed_E = BLAKE3("PROTOGAEA/EPOCH_SEED/V0" ‖ world_id ‖ E ‖ beacon_E ‖ header_hash_{E−1})`. Here `beacon_E` is the verified value of the beacon round that follows the commitment of the spark log (§20). The PoW result is never used as a source of randomness for the ecology; otherwise naturalists could grind for a convenient outcome.

**Prohibitions in the core** (enforced by linters and code review):
- no floating-point arithmetic, no system time, no unregulated parallelism;
- no iteration over hash tables, because their traversal order can change from run to run; only ordered structures and sorted arrays are used;
- overflow is handled explicitly, by saturation or checking. For example, by default Rust panics on overflow in debug builds and silently wraps in release builds. Overflow in a checked operation is a consensus error that halts the season;
- integer division is applied only to non-negative numbers or with an explicitly defined rounding rule.

**One core, three builds:** the server, the command-line replay and WASM for the browser. There is no second core in JavaScript: its numbers are exact only up to 2⁵³. A second independent implementation (for example, a reference one in another language) helps validate the specification itself; it is planned after the MVP.

**Cross-platform checks in CI** run on every commit. Platforms: x86-64 Linux and Windows, ARM64 Linux and macOS, WASM in Chrome, Firefox and Safari. For several seeds, `state_root` is compared over at least 500 epochs.

**Core performance targets:**
- one world day (3,456 ticks, 6,000 organisms) computes in under 30 s on a single reference core;
- an epoch computes on the server in under 100 ms at the 95th percentile;
- WASM is at most 3 times slower than the native build.

This speed matters less for the server than for the balance harness (thousands of seasons), counterfactuals and forecasts in the browser.

## 15. State, snapshots and the log

**The canonical state** contains only what affects the future:
- cells;
- living organisms, ordered by ID;
- living clades;
- the bounded museum and the spore bank;
- active effects;
- the epoch number, `ruleset_id`, and the `next_organism_id` and `next_clade_id` counters.

History — dead organisms, family trees, events — is kept in the log and in derived indexes, not in the state. This keeps snapshots bounded: about 350 KB uncompressed with 6,000 organisms.

**`state_root`** is the root of a Merkle tree over sorted leaves: cells by index, organisms by ID, clades by ID, the museum, global fields. Each kind of leaf has its own domain prefix. This makes it possible to prove that a specific organism existed in a specific epoch right inside a link to the history, and to download the state in parts.

**The wish state** is kept separately and hashed into `ledger_root` (§19): open wishes, accumulated work, the price, the queue of ready wishes.

**The epoch header** is signed with the operator's key and contains:
- the epoch number and the hash of the previous header;
- `ruleset_id`, `state_root`, `ledger_root`;
- the final signed tree head of the spark log;
- the number and value of the beacon round;
- the list of applied miracles and `events_root`;
- a reference timestamp, which is not used in any computation.

**The log** is an append-only chain of headers and event records. Snapshots are published every 12 epochs (once an hour), as well as for genesis and the season finale. The client recomputes intermediate states itself.

**Anchoring in time.** Once an hour, the hash of the latest header is anchored in Bitcoin through OpenTimestamps, at no cost. A rewritten history will then be detected even if nobody downloaded the log at the time. The log and snapshots are mirrored in at least two independent places: a public repository and object storage with a CDN.

**Check:** a replay from scratch on different operating systems and in the browser must produce identical roots.

# Part III. The spark protocol v0

Exact byte formats, field order and test vectors go into a separate protocol specification, written before the public client is released. This part fixes the mandatory decisions.

## 16. PoW: algorithm and hardware

- **Candidate: yespower 1.0** with `N = 2048`, `r = 32` (about 8 MiB of working memory per thread) and the personalization string `pers = "PROTOGAEA/SPARK/V0"`. Domains are separated at the level of the algorithm itself, not only in the input data. The final decision is made after portable benchmarks on x86-64, ARM64 and available GPUs, after fixing test vectors, and after reviewing the license of the specific implementation.
- **Honest wording.** The author of yespower calls the algorithm CPU-friendly and GPU-unfriendly, but acknowledges that ASICs may gain an advantage. We do not promise that GPUs and ASICs are "technically impossible". The goal is conditions that, *as measured*, favor ordinary processors at launch. The absence of an absolute guarantee is stated publicly.
- **Portability.** The spark client runs on ARM64 without a mandatory JIT and without x86-specific instructions. SIMD is only an optimization with a portable fallback path.
- **Verification cost.** Verifying one spark costs the server on the order of milliseconds and 8 MiB of memory. The exact figure is measured and published, because the intake budget depends on it (§18).

## 17. Wish (proposal): format and validation

- **The canonical bytes of a wish** are a fixed binary format with these fields:
  - version, `world_id`, `ruleset_id`;
  - action type and parameters;
  - `author_pubkey`, `created_epoch`, `expires_epoch`;
  - an optional hypothesis — template ID and parameters;
  - an optional name — indexes into the root dictionary.
- **Identifier:** `proposal_id = BLAKE3("PROTOGAEA/PROPOSAL/V0" ‖ canonical_bytes)`. The signature is not part of the identifier, so any signature malleability changes neither the ID nor the work bound to it.
- **Signature:** Ed25519 over `proposal_id` with strict verification: canonical `S`, rejection of small-order points. The exact rule is fixed by test vectors shared by all implementations.
- **Creation:** `POST /v0/proposals` with the wish, its signature and its first spark.
- **Limits:** at most 3 open wishes per author key; a lifetime of at most 288 epochs.
- **Intake checks:** format → signature → validity of the parameters against the latest published state → limits → first spark. The hard check at application is described in §5.

## 18. Sparks: format, intake, receipts

**Epoch challenge:** `challenge_E = BLAKE3("PROTOGAEA/CHALLENGE/V0" ‖ world_id ‖ E ‖ header_hash_{E−1})`. It becomes known only after the previous epoch's header is published, so work cannot be accumulated in advance.

**PoW input** (byte order is fixed by test vectors):

```
"PROTOGAEA/SPARK/V0" ‖ world_id (16 B) ‖ epoch (u64 LE) ‖ challenge_E (32 B)
‖ proposal_id (32 B) ‖ miner_pubkey (32 B) ‖ nonce (u64 LE)
```

The first 8 bytes of the yespower output, read as a u64 (big-endian), must be less than the epoch target `t_E`. A spark's weight in work units is `floor(2⁶⁴ / t_E)`, i.e. the expected number of hash evaluations.

**Sparks are not signed.** The naturalist's key is part of the PoW input, so nobody can claim someone else's work: with a different key the proof becomes invalid. This saves the server a signature check on every spark. Sparks are submitted in batches of up to 64 as `{proposal_id, miner_pubkey, nonce}` — 72 bytes per spark.

**Intake check order:**
1. Size and format.
2. The epoch window is open.
3. The wish exists and is open.
4. The `spark_id` has not been seen before.
5. Limits per IP, subnet and key.
6. **One** PoW verification.
7. Append to the spark log.
8. Receipt.

**The epoch target `t_E`** changes only between epochs, never in the middle of a window. It adjusts to the number of verified sparks: the reference point is about 20,000 sparks per epoch across the whole network, with steps of at most ±25%. The target bounds the verification load and does not affect the price of a miracle: the price is measured in work units, not in sparks.

**Protection against cheap attacks.** An invalid PoW costs the sender nothing, while verifying it costs the server milliseconds and 8 MiB. Therefore:
- rate limiters (token buckets) per IP, subnet and key run before PoW verification;
- an invalid PoW gets the key and the IP temporarily blocked (candidate: for 1 hour). An honest client verifies each spark locally and never sends an invalid one;
- the verification queue is bounded; when overloaded, the server responds with `E_OVERLOADED` and a `Retry-After` header;
- verification uses at most 2 cores under normal load;
- arrival order within a window gives no advantage: only the total work counts.

**The spark log and receipts** follow the Certificate Transparency design (RFC 6962).
- Each epoch's sparks are appended to a Merkle tree. A signed tree head (STH: epoch, size, root, signature) is published at least every 2 seconds.
- A receipt contains the `spark_id`, the leaf index, the STH that includes the spark, and an inclusion proof.
- The API serves consistency proofs between any two STHs, so the removal of a previously confirmed spark is detectable.
- A receipt is issued only after the spark has been written to the log.

## 19. The price of a miracle and the execution queue

All quantities are integers (work is stored as u128). The computation is deterministic: any observer can repeat it from the final STH of the spark log and the beacon value.

1. After the window of epoch E closes, the weight of each open wish's sparks from the epoch's final tree is added to that wish's accumulated work: `W_p += Σ weights of p's sparks`.
2. Wishes whose lifetime has ended are closed as `expired`. Wishes that stopped being valid by the time the window opened are closed as `invalidated`.
3. The price of a wish: `price_p = P_E × price_mult[action_p] / 100`. The `price_mult` coefficients are integers from the `ruleset`; candidates: `weather` 100, `migrate` 120, `revive` 200. A wish is ready if `W_p ≥ price_p`; wishes deferred earlier are ready as well.
4. Ready wishes are sorted in descending order of the share of the price they have covered, i.e. by `W_p / price_mult[action_p]`. Values are compared by cross-multiplication, without division. Ties are broken by `BLAKE3(beacon_E ‖ proposal_id)`.
5. Up to 3 wishes are selected in that order. A wish that conflicts with one already selected is deferred (§5).
6. The selected wishes are applied at the epoch E boundary (§13) and closed as `executed`.
7. The price for the next epoch:
   - if ready wishes remain in the queue after selection, `P_{E+1} = P_E + P_E / 8`;
   - if fewer than 3 wishes were selected, `P_{E+1} = max(P_min, P_E − P_E / 8)`;
   - otherwise the price does not change.
8. The wish state (open wishes with their `W_p`, the price, the queue) is reduced to a Merkle root, `ledger_root`, which is included in the epoch header.

**The price floor `P_min`** is set before the season starts. Candidate: about 8 core-hours of work on the reference core. The equivalent in work units is determined by benchmarks on the reference processor and published together with its description.

The order in which sparks arrive within a window does not affect the result, so the operator cannot gain anything by reordering them.

## 20. Epoch timeline

Windows close on a schedule every 300 s (for example, at :00, :05, :10…). Epoch E goes through six steps:

| Step | When | What happens |
|---|---|---|
| 1. Window opens | Right after the header of E−1 is published, about 15 s after window E−1 closes | `challenge_E` becomes known; wishes get a soft check against the state at the end of E−1; spark intake begins |
| 2. Close | On schedule, at `close_E` | Sparks for epoch E are no longer accepted |
| 3. Commitment | No later than `close_E + 2 s` | The final signed STH of the epoch E spark log is published; watchers record when they saw it |
| 4. Beacon round | The first round with a time no earlier than `close_E + 10 s` | The round number is fixed in advance by a rule; its value is verified against the beacon network's signature |
| 5. Computation | About `close_E + 12…14 s` | Work, price and miracle selection (§19) and the epoch seed (§14) are computed; the epoch boundary and 12 ticks run (§13) |
| 6. Publication | About `close_E + 15 s` | The epoch E header, its events and, when due, a snapshot are published; window E+1 opens |

- **A window lasts about 285 s.** If the previous epoch's header is late, the close is moved so that the window lasts at least 120 s, and the schedule shifts.
- **What viewers see.** While window E is open, the screen replays epoch E−1 (§7).
- **Which state miracles apply to.** Miracles selected when window E closes open epoch E. They are applied to the same state that naturalists based their decisions on. So only wishes that became invalid before the window opened get the `invalidated` status, and they are closed before anyone spends sparks on them.
- **World time is logical.**
  - If the beacon is delayed, the epoch waits for its value and accepts no new sparks. The operator never picks a "convenient" seed.
  - If the server is down, the world stands still and does not "catch up" afterwards: the schedule continues from the next epoch number.
  - Season events (for example, land bridges closing) are tied to epoch numbers and shown in the interface with an approximate time.
- **Beacon network.** Candidate: drand's quicknet, with a round every 3 s. That the network is still operating, and the exact round selection rule, are verified and fixed before the season (§31).

## 21. Verifiability and the trust boundary

**What any observer can verify without access to the server's database:**
- that each spark matches its challenge, target and wish;
- that the epoch's final STH is consistent with every receipt issued earlier — via consistency proofs;
- the computation of work, price, queue and miracle selection — from the spark log and the beacon value;
- the entire simulation — from genesis, the rules and the miracle log: `state_root` must match at every epoch;
- that the log has not been rewritten — from the header chain and the OpenTimestamps anchors.

**Committing before the beacon keeps the operator from steering outcomes.** Receipts make it possible to notice that the operator dropped someone else's sparks, but they do not stop the operator from adding its own. If the final set of sparks were published after the beacon value, the operator could hold back honestly produced sparks and include only the favorable ones. That is why the final STH is published before the beacon round, and independent watchers record when they saw it. An STH that first appears after the round time is a violation visible to everyone.

**Watchers.** The watcher program is open source. It follows STHs and headers, checks their consistency, repeats the replay and reports discrepancies. By the public season, at least two watchers run by parties other than the operator must be operating.

**The v0 trust boundary**, stated publicly:
- the operator can refuse someone's spark; receipts and watchers make this detectable but do not prevent it;
- the operator can delay publication or stop the world; this is visible but not prevented;
- the operator can kindle sparks too, on the same terms as everyone else;
- spark intake is not decentralized. The decision about a network of independent nodes is made after the MVP (§30, stage E).

# Part IV. Architecture, platforms, operations, openness

## 22. Architecture v0

v0 runs **a single authoritative world server**. Everything it publishes can be verified independently.

| Component | Role | Part of consensus |
|---|---|---|
| `epoch scheduler` | Window schedule; runs the steps of §20 | No |
| `spark service` | Intake of sparks and wishes, PoW verification, the spark log with STHs, receipts | The spark log |
| `ledger` | Work per wish, price, queue, miracle selection | Yes (`ledger_root`) |
| `beacon verifier` | Fetching and verifying the beacon round | The beacon value |
| `simulation core` | A pure deterministic function in Rust; built for the server, the command line and WASM | Yes (`state_root`) |
| `event log` | Headers, events, snapshots, mirrors, OpenTimestamps anchoring | Yes |
| `read API` | Indexes, cards, history, proofs | No |
| `story engine` | Story detectors, chronicle, hall of fame | No |
| `counterfactual worker` | Shadow worlds for miracles, 36 epochs ahead | No, but the result is reproducible |
| `notifier` | Web push, Telegram bot, digests | No |
| `media renderer` | Link previews, GIFs and video from the replay, the stream | No |
| `watcher` | Watcher: STHs, headers, replay | No; runs independently |
| Clients | Browser viewer, desktop app, WASM spark client | No |

**Isolation.** A failure in the web interface, the chronicle generator or the renderer cannot damage the world log. Images, texts and visualizations are not part of the state. Everything outside consensus can be recomputed from the log.

**Recommended stack:**
- **Rust** — the core, the server and the spark client. For yespower: the reference C implementation via FFI, or a Rust port that passes the same test vectors.
- **TypeScript** with a WebGL renderer (for example, PixiJS) — the viewer.
- **Tauri** — the desktop app.
- **SQLite or PostgreSQL** — indexes.
- **Object storage and a CDN** — snapshots and the log.

**A dedicated distributed network is a decision for after the MVP.** For a network to agree on the world by itself, every node would have to verify PoW, event order, randomness, full recomputation and `state_root`, and resolve competing branches. That is a separate protocol with its own security analysis. The public log, watchers and replay let us first find out whether people are interested in the game.

## 23. API v0

**Read:**
- `GET /v0/world`, `/v0/ruleset`, `/v0/epochs/{n}` (header), `/v0/snapshots/{epoch}`;
- `GET /v0/organisms/{id}`, `/v0/clades/{id}`, `/v0/museum`, `/v0/events?cursor=`;
- `GET /v0/proofs/{epoch}/{kind}/{id}` — a proof that an organism, cell or clade is included in `state_root`;
- `GET /v0/proposals?status=`, `/v0/proposals/{id}`, `/v0/miracles/{id}`, `/v0/counterfactuals/{miracle_id}`;
- `GET /v0/sparks/sth/{epoch}`, `/v0/sparks/consistency?from=&to=`, `/v0/sparks/{spark_id}/receipt`;
- `GET /v0/chronicle/{day}`, `/v0/attention` (the heat map).

**Write:**
- `POST /v0/proposals` — a wish, its signature and its first spark;
- `POST /v0/sparks` — a batch of up to 64 sparks;
- `POST /v0/rings`, `POST /v0/predictions`, `POST /v0/subscriptions` — requests signed with the user's key; not part of consensus.

**Error codes** are distinct from each other and understandable to humans: `E_FORMAT`, `E_WINDOW_CLOSED`, `E_UNKNOWN_PROPOSAL`, `E_PROPOSAL_CLOSED`, `E_DUPLICATE`, `E_RATE_LIMIT`, `E_POW_INVALID`, `E_SIGNATURE`, `E_ACTION_INVALID` (with a reason), `E_BANNED`, `E_OVERLOADED`.

**General rules.** Cursors are stable, page sizes are bounded, and unknown fields are handled according to the version's published rules. Bulk export through the API is limited, but a complete archive of snapshots and the log is available separately for replay.

## 24. Platforms, clients and keys

**The viewer** runs in the browser on desktop and mobile. The WASM core powers the time machine, counterfactuals, forecasts and root verification. Phones get a lightweight mode. iOS is view-only: App Store rules forbid this kind of computation on the device.

**The desktop app** (for example, built with Tauri) combines the viewer, the spark client and key storage, so there is no need to connect a web page to a separate program.

| Platform | Status for the public season |
|---|---|
| Windows x86-64 | Required |
| Linux x86-64 | Required |
| macOS arm64 (Apple Silicon) | Required: the largest desktop ARM64 audience |
| Linux ARM64, including Raspberry Pi 4/5 | Required. For the Pi, a headless mode with a local page: a "world altar" that is quiet, frugal and on around the clock |
| macOS x86-64 | If the build is cheap |
| Android | Out of the MVP: Google Play rules forbid this kind of computation on the device, and it drains the battery. An experiment outside the store is possible later |

**The browser spark client (WASM)** lets people start without installing anything:
- it starts only with an explicit button, and a visible indicator shows it is working;
- it runs only while the tab is open;
- threads use Web Workers, about 8 MiB of memory per thread;
- it is less efficient than the native client, and the user sees the measured rate.

**Keys.**
- Ed25519. The key is created in the app or in the browser.
- The key is stored locally: in the OS keychain in the app, and as a non-extractable key in the browser where supported.
- A key moves to another device via a QR code or a pairing code with confirmation. Per-device subkeys come after the MVP.
- Key creation says plainly: losing the key means losing the profile's history. A backup can be saved to an encrypted file.

**Releasing builds** (licenses and the publicity timeline are in §26):
- open source code and reproducible builds;
- code signing: Authenticode on Windows, Apple notarization;
- builds are sent to antivirus vendors for review in advance;
- file and process names never contain the word "miner";
- official builds are published only through the repository's [GitHub Releases](https://github.com/protogaea/protogaea/releases), with checksums and signatures. Builds from any other source are unofficial.

## 25. Operations, load, abuse

- **World growth.** Every level has upper limits: the number of organisms, clades, wishes and sparks, request sizes, verification time. The core's load is measured before choosing a server.
- **Hostile clients.** Malformed sparks, millions of replays, huge requests, wish spam, invalid signatures, invalid PoW — the defenses are described in §18. Cheap checks run before expensive ones. A receipt is issued only after the write to the log.
- **A dishonest operator** — §21. The trust boundary is announced publicly before the season begins.
- **Beacon unavailability.** The epoch waits for the value, monitoring reports the reason for the delay, and the log keeps serving reads.
- **Replay divergence.** If a watcher or our own replay gets a different `state_root`, the season is halted until the cause is found, and a report is published.
- **Seasons and bugs.** Rules are never fixed quietly. A bug in the core means halting the season, publishing a report and issuing a new `ruleset_id`. The old log remains available for analysis.
- **User content.** There is no free text: names are assembled from a vetted dictionary of roots, and hypotheses from templates. If something is hidden from display, the event still stays in the verifiable log, except in cases described by a published rule.
- **Privacy.** No registration; keys are pseudonymous. IP addresses are stored only for rate limiting and for no longer than 7 days.
- **Monitoring:**
  - duration of the epoch steps;
  - beacon delay;
  - the spark verification queue;
  - the share of invalid PoW;
  - the number of running watchers;
  - replay divergences.
- **Cost and sustainability.** v0 needs one server with 4–8 cores, object storage and a CDN. Donations are accepted but give no influence over the world or the odds. The long-term way to pay for the project without a token is private worlds for education (after the MVP).

## 26. Openness, licenses and language

**Principle.** The code is open from the first commit, while the loud announcement comes only after stage B, when there is something to show. Openness of the code and access to the world are decided separately: the code is open immediately, while the world is closed in the early stages (the B′ test is invitation-only).

**Why the code is open from the start:**
- The trust model requires it. Replay, watchers and root verification in the browser mean nothing if the core and the client are closed: a closed client can display "✓ verified" without verifying anything. The code would have to be opened by stage C anyway, and a commit history from day one shows that the rules were never changed quietly.
- There is nothing to hide: the rules, the rift schedule and the seed are published before the season (§4, 9).
- An open reference spark client levels the field: speed-ups reach everyone, not just those who would write a private fast version for themselves.
- Open code and reproducible builds are the main defense against antivirus alerts and suspicions of hidden mining (§24).
- The audience — artificial-life enthusiasts, researchers, teachers — values openness. The repository and a development diary help recruit participants for B′.

**Licenses:**

| What | License | Why |
|---|---|---|
| Simulation core, `ruleset`, protocol specification, replay, watcher, spark client | Apache-2.0 | Maximum freedom for independent verification, ports and a second implementation; an explicit patent grant |
| Server and web application | AGPL-3.0 | Anyone who runs a public copy as a service must publish their changes; this protects the future hosting of private worlds for education |
| World log, snapshots, season archives | Open data: CC0 or CC BY 4.0 (to be decided before the season) | Researchers and archivists can freely use and mirror the world's history |
| Root dictionary, text templates, artwork | CC BY-SA 4.0 (candidate) | Reusable with attribution |
| Name and logo | Not licensed | Trademark usage rules apply: a fork must change its name |

**What stays closed:**
- the operator's keys and other secrets;
- abuse-protection thresholds and infrastructure configuration; only the general protection scheme is published;
- the world during stage B′: invitation-only access.

**Contribution rules:**
- External PRs are not accepted until the core stabilizes. The README says plainly: "pre-alpha, everything will break". Bug reports and discussions are welcome.
- A CLA or DCO is chosen before the first external contribution. Without a CLA, the license of code already accepted from others cannot be changed later.
- The code and the specification live in the public repository [github.com/protogaea/protogaea](https://github.com/protogaea/protogaea); the specification is in `docs/spec/`.

**Publicity timeline:**

| Stage | Code | World | Announcement |
|---|---|---|---|
| A | Open | None | None |
| B | Open | A demo for the usability test | A quiet development diary |
| After B | Open | — | Loud announcement and recruitment for B′ |
| B′ | Open | Closed, invitation-only test | — |
| C | Open; reproducible signed builds | Closed test with sparks | — |
| D | Open | Public Season 1 | Public launch |

**The repository.** The public repository [github.com/protogaea/protogaea](https://github.com/protogaea/protogaea) was created on 2026-09-24 under the author's account and moved to the `protogaea` GitHub organization on 2026-09-25; the old address, github.com/danifest751/protogaea, redirects to it. Its About description and topics are set. Because the repository is already public, anything pushed to it is published immediately.

**As soon as possible**, since the repository is already public:
- reserve the rest of the name: the domain protogaea.com with its main spelling variants, and package names on crates.io and npm (the `protogaea` GitHub organization is already reserved);
- have a lawyer check the final name — the public repository already ties it to the project;
- fill in the contacts in the Code of Conduct, SECURITY and TRADEMARKS files, and enable private vulnerability reporting in the repository settings.

**Before the loud announcement (after stage B):** approve the trademark usage rules and the public "no token" stance.

**Done:** the specification was translated into English on 2026-09-25; this document is the canonical English version.

**Result of the preliminary name check** (2026-09-24; not a legal opinion):
- **"Pangea" as a product name is high-risk.**
  - Steam has at least four games with the word in the title (for example, Pangea Survival, 2020); there are also board games about Pangea.
  - In the EU, according to the registry check, the marks "Lands Of Pangea" and "PANGEAN GAMES" are in force in the video-game classes; the second also covers crypto and NFTs.
  - The name is used by crypto projects, for example the Pangea Swap exchange with its STONE token.
  - The main domains and the `pangea` login on GitHub are taken.
- **In Russia** the marks «ПАНГЕЯ» and "PANGEA" belong to JSC Pangeya, but in the classes of instruments and geological exploration, so the risk there is low.
- **The chosen working name is Protogaea** (Russian «Протогея», "proto-Earth"; the title of Leibniz's treatise on the early Earth). No marks or products with this name were found; protogaea.com and the `protogaea` login on GitHub are free. Downsides: it is not easy to pronounce in English, and spelling variants need to be secured (`Protogea` is already taken on GitHub).
- **Fallbacks:** Pangea Wilds and Pangea Drift. They are free, but the EU trademark risk remains.
- **"Pangea" remains the name of the Season 1 supercontinent in any case** ("The Breaking of Pangea"). A season title is part of the world, not a trademark.
- **Decision made on 2026-09-25:** the working name is Protogaea («Протогея»). A lawyer checks the final name in classes 9, 41 and 42 (plus 28 if there is merchandise) in the US, the EU, the UK and Russia.

**Forks.** Sooner or later someone will make a fork with a token; a closed start would only have delayed it. The defenses are the trademark, the public "no token" stance, and the fact that the living world, with its history and community, stays with the project.

**World handover plan (candidate).** The world must not die with the operator's server: that is how the networked modes of several earlier projects ended (Appendix E). Before Season 1, a commitment is published: if the operator stops supporting the world, it releases a final archive (log, snapshots, rules) and allows the community to continue the world from the last snapshot, including under the project's name. A lawyer checks the exact wording together with the trademark usage rules.

**Language:**
- **English for everything outsiders read and everything long-lived:** code, identifiers, comments, commit messages, issues and PRs, the API, the protocol and rules specifications, the README. From the first commit — switching later is expensive.
- **Documentation.** The canonical version is English. The Russian version is kept as a translation and updated at releases; if they differ, the English version prevails.
- **The interface** is localized from day one. English is the default language and Russian the second language from the very start. All strings, including templates for events, the chronicle and notifications, live in resource files, not in code.
- **Clade names** are Latin binomials and need no translation.
- **English world terms** are the candidates in §2; native speakers review them before stage B. Interface sections: While you were away, Chronicle, Museum, Spore bank. Season 1 is The Breaking of Pangea.
- **Community.** The main language is English; a Russian-speaking channel runs separately.

# Part V. Acceptance and stages

## 27. Functional acceptance criteria

1. The world runs live for at least 72 hours with no player actions. On the balance harness, the metrics of §28 are met on at least 90% of 200 seeds over a full season.
2. An end-to-end scenario with two testers:
   - tester A creates a `migrate` wish, and tester B supports it;
   - their sparks from x86-64 and ARM64 are accepted, and each one gets a receipt;
   - the wish reaches its price and executes;
   - both see the miracle on the map together with its counterfactual, and both recompute the selection themselves from the spark log and the beacon value.
3. An observer downloads the genesis, the rules, the log and the snapshots and, without access to the server's database, reproduces at least 500 consecutive epochs with identical `state_root` values. The replay works natively on Windows, Linux and macOS, and in the browser.
4. Each of these errors is rejected with its own understandable code (§23): a duplicate spark, a modified wish, a late spark, an invalid signature, an invalid PoW, an invalid action, a closed wish.
5. Permanent links and proofs:
   - every epoch, event, clade, organism and miracle has a permanent link;
   - after a reload, the client correctly restores a past state;
   - an organism's existence in an epoch is confirmed by an inclusion proof.
6. The "While you were away" digest and subscriptions (web push or Telegram) work.
7. The Muller plot and the museum cover the entire season.
8. Rifts change strictly according to the published schedule. Subscribers learn about each land bridge closing at least one day in advance.
9. For every epoch, a watcher sees the final STH before the beacon round time.

## 28. Ecosystem health: the balance harness

The harness is a program that runs thousands of seasons offline with different seeds and parameter sets and computes metrics. It is built in stage A. The `ruleset` parameters are chosen based on its results, and the results are published together with the season rules.

| Metric | Pass condition (candidate) |
|---|---|
| Total extinction without players before the season ends | At most 5% of seeds |
| Spore bank triggers | A median of at most once per season |
| Coexisting clades of ≥ 20 organisms | At least 6 at any time after the third day |
| Change of the world's dominant clade | A median of at least once every 3 days. No clade holds more than 60% of the population for longer than 3 days |
| Predator–prey population oscillations | Detected on at least half of the seeds |
| Diversity across biomes | Different archetypes dominate in at least 3 biomes |
| Share of ticks at the global limit | Under 1% |
| Equilibrium population | 40–70% of the limit |
| Pace of evolution | A median of at least 30 generations per world day |
| Divergence after the breakup | By the end of the season, the mean distance between the dominant clades of different continents grows by at least 3 steps |
| Archetype arena | In pairwise tests of the three archetypes (§11.3), each beats one opponent and loses to the other |
| Performance | One world day computes in under 30 s on the reference core |

There is no goal of "ever-increasing complexity". Open-ended evolution remains a hard research problem: even in classic experiments, organisms growing larger is not necessarily accompanied by growing complexity (Appendix D).

## 29. Engineering and product checks

| Check | Pass condition |
|---|---|
| Cross-platform agreement | PoW test vectors and 500 epochs of replay match on x86-64, ARM64 and WASM |
| Performance | A core epoch computes in under 100 ms (p95). From window close to header publication takes under 20 s (p95), including the wait for the beacon |
| Intake resilience | Under 10,000 invalid sparks per second from 1,000 addresses, honest sparks are still accepted and verification stays within 2 cores |
| Verifiability | Receipts and watchers detect the disappearance of a confirmed spark. Every epoch's STH is visible before the beacon round. The trust boundary is described publicly |
| Antivirus | At release, major antivirus products do not flag the signed builds |
| Audience interest | The metrics of stages B′ and D reach the targets declared before the test |

**Product metrics for stages B′ and D:**
- the share of viewers who return on day 1 and day 7, with and without reminders;
- the share of viewers who opened a clade card or the Muller plot;
- how many stories were shared per week and how many visits they brought;
- the share of wishes supported by at least two people;
- the share of returning users who read the "While you were away" digest;
- the number of subscriptions per active user.

Target values are fixed and published **before** the test starts, not fitted to the results. Success is measured in returning users and meaningful stories, not in hashes.

## 30. Implementation stages

| Stage | Deliverable | Exit condition |
|---|---|---|
| A. Core and balance harness | A deterministic Rust core with a WASM build, cross-platform CI, the balance harness with metrics, the map generator and the Season 1 rift schedule | The §28 metrics are met; roots match on all platforms; performance targets are reached |
| B. Observation | Map, Muller plot, clade tree, cards, the WASM time machine, story detectors, museum, automatic names, "While you were away", subscriptions | In a usability test, 5–8 people explain the consequences of a mutation or an event without looking at server logs |
| B′. Closed observation test | 2–3 weeks, 30–100 invited participants. The full wish mechanic, but instead of PoW everyone gets a daily allowance of work units | The metrics declared before the test are reached. If viewers do not come back, we improve the world, not the sparks |
| C. Sparks | yespower; the desktop app for Windows, Linux, macOS arm64 and Linux ARM64; the WASM spark client; the spark log with STHs and receipts; `ledger`; beacon; watcher; load tests; signed builds | Sparks and miracle selection are verified independently; the §29 checks pass |
| D. Public Season 1, "The Breaking of Pangea" | 42 world days; counterfactuals, chronicle, stream, hall of fame | No state divergence; real people create and discuss stories; product metrics |
| E. Decision on further development | An assessment of retention, the cost of servers and sparks, and trust in the operator | Choose a direction based on the results: a network of independent nodes, private worlds for education, a deeper game (sexual reproduction, evolving behavior), themes for the next seasons |

The spark source is a separate module. The stage B′ stub can be replaced with PoW without reworking the other parts.

## 31. Decisions to make before writing code

1. **PoW.** The final algorithm and its parameters are chosen after benchmarking computation and verification speed on x86-64, ARM64 and available GPUs. The absence of an absolute "CPU-only" guarantee is stated publicly.
2. **World rules.** The `ruleset` numbers come from the balance harness results. The Season 1 seed is chosen by published criteria.
3. **Randomness.** The beacon network (candidate: drand quicknet), the exact "window close → round number" rule, and the rule for delays. At least two independent watchers are running.
4. **Formats and publication.** Formats of receipts, STHs and headers; where the log is mirrored; how often it is anchored through OpenTimestamps.
5. **Names and hypotheses.** The root dictionary for names and the hypothesis templates.
6. **Price of a miracle.** A description of the reference core and the value of `P_min` in work units.
7. **Metrics.** Target values of the product metrics for stages B′ and D are fixed before the tests begin.
8. **Legal.** A legal review of the wording and of distributing the spark client in the target countries, of app store rules, and of user consent requirements.
9. **Name.** A preliminary check has been done (§26): "Pangea" is high-risk, and the working name "Protogaea" («Протогея») was chosen. The public repository [protogaea/protogaea](https://github.com/protogaea/protogaea) has existed since 2026-09-24, so the remaining steps are urgent: a lawyer's check, and registering the domains and namespaces together with spelling variants. If the check finds a conflict, the name changes or gets a distinguishing addition.
10. **Licenses and contributions.** The final data license (CC0 or CC BY 4.0); the choice between a CLA and a DCO before the first external contribution; the trademark usage rules.
11. **English version.** Done on 2026-09-25: this document is the canonical English version, and the Russian text is its translation. Native speakers still review the English world terms before stage B.

# Appendices

## A. Glossary

| Term | Russian | Meaning |
|---|---|---|
| Protogaea | Протогея | Working name of the world and the product; "proto-Earth" |
| Pangea | Пангея | The Season 1 supercontinent; it breaks apart on a published schedule (§4) |
| Tick | Такт | The smallest simulation step |
| Epoch | Эпоха | 12 ticks, nominally 5 minutes. The unit of the log, of the spark intake window and of miracle application |
| World day | Сутки мира | 288 epochs |
| World year | Год мира | 372 epochs (31 hours), made up of four times of year |
| Time of year | Время года | Spring, summer, autumn or winter within a world year; affects food growth |
| Season | Сезон | A game season with fixed rules and its own `ruleset_id`. Season 1 is "The Breaking of Pangea", 42 world days |
| Founder lineage (`lineage_id`) | Линия основателя | All descendants of one starting or revived group |
| Clade (`clade_id`) | Клада | A branch of the family tree with a reference genome; the main unit of the interface |
| Naturalist | Натуралист | A profile bound to a public key |
| Wish (`proposal`) | Замысел | A public proposal for an intervention that accumulates sparks |
| Spark (`spark`) | Искра | A proof of work for a wish, accepted by the server. Its weight is the expected number of hash evaluations |
| Miracle (`miracle`) | Чудо | An executed wish |
| Price of a miracle (`P_E`) | Цена чуда | The epoch's base price. A specific wish's price is the base price times its action's coefficient (§19). `P_min` is the floor of the base price |
| Window | Окно | The spark intake period of an epoch, about 285 s |
| STH | STH | Signed tree head of the spark log |
| `state_root` | `state_root` | Merkle root of the canonical world state |
| `ledger_root` | `ledger_root` | Merkle root of the wish state |
| Spore bank | Банк спор | Genomes from genesis and from clades that survived to the previous season's finale. The source of natural revival and of the `revive` action |
| Museum | Музей | Extinct named clades. The latest 1,024 are kept in the state and can be revived with `revive` |
| Counterfactual | Контрфакт | A shadow world without a specific miracle, computed with the same rules and the same randomness |
| Ensemble forecast | Ансамблевый прогноз | The distribution of outcomes over many runs of the future with random seeds |
| Watcher | Наблюдатель | An independent program that records STHs and headers and cross-checks the replay |
| Reference core | Эталонное ядро | A described and published processor used to calibrate the price of a miracle and the performance targets |

## B. Decision log

| Decision | Chosen | Rejected | Why |
|---|---|---|---|
| Allocating influence | Wishes that accumulate work; a price; at most 3 miracles per epoch | The v0.1 ticket lottery | With a large audience the odds are negligible (≈0.9% per 15 minutes with 1,000 participants); with a small one, interventions are nearly free. No cooperation. Undefined behavior when two tickets of the same proposal win |
| How new lineages appear | `revive` from the museum or spore bank with at most 2 edit steps | `found_lineage` with an arbitrary genome | The rules are open and the world can be simulated locally, so a free choice of genome turns into min-maxing optimal builds |
| Randomness | Counter-based | A stateful generator | Independent of call order, allows parallel computation, makes counterfactuals honest |
| State | Only what affects the future; a Merkle root | Including lineage history; a flat hash of the whole state | Bounded size; an organism's inclusion in the state can be proven |
| Committing sparks | The STH is published before the beacon round | The list is published after the window closes, without a timestamp | Otherwise the operator could include only its own favorable sparks |
| Length of a year | 31 hours | A multiple of a day | Otherwise a viewer who visits at the same hour always sees the same time of year |
| Interface vocabulary | Sparks, wishes, miracles | "Mining", "tickets" | Antivirus software, app store rules, crypto associations |
| A spark client on phones | Out of the MVP | Android ARM64 in the MVP | Store rules and battery drain. The ARM64 platform is covered by macOS and Raspberry Pi |
| A separate core in JavaScript | No: one core compiled to WASM | A separate implementation for the browser | JavaScript numbers are exact only up to 2⁵³; two implementations are harder to maintain and easier to drift apart |
| A network of nodes | After the MVP | Right away | It is a separate protocol with its own security analysis; interest in the game has to be tested first |
| Code openness | Open from the first commit; loud announcement after stage B | Closed code until launch | The trust model (replay, watchers, verification in the browser) does not work without open code; there is nothing to hide; an open spark client levels the field |
| Licenses | Apache-2.0 for everything needed for verification; AGPL-3.0 for the service; the name protected as a trademark | One license for everything | Those who verify the world need maximum freedom, while public copies of the service must share their changes |
| Project language | English for code and documentation; English interface by default, Russian from day one | Russian only, translation "later" | The English-speaking audience is larger; translating identifiers and project history later is expensive |
| Product name | Protogaea («Протогея»); Pangea is the Season 1 supercontinent | "Pangea" as a brand; Pangea Wilds, Pangea Drift | "Pangea" is high-risk: games on Steam, EU trademarks for video games, crypto projects, taken domains. Variants with an addition keep the EU trademark risk. "Protogaea" is free and covers all future seasons |

## C. Risks

| Risk | Consequence | Mitigation |
|---|---|---|
| The ecology freezes or dies out | Without sparks, the world is not interesting to visit | The balance harness, the yearly cycle, natural events, "kill the winner" plague, the archetype arena. Parameters change only between seasons |
| Viewers do not come back | The main hypothesis fails | Stage B′ runs before sparks are built; the "While you were away" digest, subscriptions, the chronicle |
| Antivirus software flags the client | People abandon installation | Code signing, open code, reproducible builds, advance review with antivirus vendors, a no-install WASM entry point |
| App store rules | No spark client on phones | In the MVP, phones are view-only |
| The legal status of "mining" | Restrictions in some countries | A legal review before the season; no token and no income; in-world terminology |
| Owners of large computing power | Their wishes execute more often | Stated openly. Action limits and cooldowns protect the world; cooperation among everyone else provides a counterweight |
| Distrust of a single operator | Doubts about the world's honesty | The spark log with an STH before the beacon, receipts, independent watchers, OpenTimestamps, open replay |
| The replay diverges from the server | The season is halted | Core prohibitions (§14), cross-platform CI, one core in all builds |
| WASM is slow on weak devices | Forecasts and the time machine lag | A lightweight mode, counterfactuals on the server, hourly snapshots |
| The language model makes mistakes in the chronicle | People stop trusting the stories | Every statement links to an event; the text is labeled as generated; by default the chronicle is template-based |
| A fork with a token under a similar name | The project is confused with a cryptocurrency; its reputation suffers | Trademark usage rules, the public "no token" stance; the living world and the community stay with the project |
| The name collides with someone else's trademark or product | Renaming after launch | Checking the name before the repository is published (§26, 31); a lawyer's advice when in doubt |
| Someone else's code accepted without a CLA | The license can no longer be changed | A CLA or DCO is chosen before the first external contribution; until then, external PRs are not accepted |
| Computing power is rented or bought (hashrate markets, botnets) | Big players' wishes execute more often | Open concentration statistics (§6), bounded effects and cooldowns, an official fast client. Capping the share of a single key is pointless: keys are free |
| The operator stops supporting the world | The world and its history are lost | The world handover plan (§26), open code, public archives and mirrors |

## D. Sources

- Avida and the properties of digital evolution: [PLOS Computational Biology](https://journals.plos.org/ploscompbiol/article?id=10.1371/journal.pcbi.1005414).
- Tierra: [What is Tierra?](https://tomray.me/tierra/whatis.html)
- Open-ended evolution and growth of complexity in Tierra: [Standish, arXiv:nlin/0210027](https://arxiv.org/abs/nlin/0210027).
- yespower: [github.com/openwall/yespower](https://github.com/openwall/yespower).
- Verifiable public randomness: [drand](https://docs.drand.love/).
- Spatial rock–paper–scissors: Reichenbach, Mobilia, Frey. *Mobility promotes and jeopardizes biodiversity in rock–paper–scissors games.* Nature 448, 2007 — [nature.com/articles/nature06095](https://www.nature.com/articles/nature06095).
- Certificate Transparency logs: [RFC 6962](https://www.rfc-editor.org/rfc/rfc6962).
- Anchoring in time: [OpenTimestamps](https://opentimestamps.org/).
- App store rules: [Google Play Developer Policy](https://play.google.com/about/developer-content-policy/), [App Store Review Guidelines](https://developer.apple.com/app-store/review/guidelines/).

## E. Comparable projects and lessons

This review is based on open sources as of 2026-09-24. No direct analog was found: no project combines a persistent public evolving world, non-monetary CPU work as a means of bounded intervention, and verifiability without a token. Individual parts of Protogaea do repeat existing projects.

| Project | Years and status | What it shares with Protogaea | Lesson |
|---|---|---|---|
| [TechnoSphere](https://en.wikipedia.org/wiki/TechnoSphere_(virtual_environment)) | 1995–2002 and 2007–2012; closed | Users designed creatures that lived, ate and bred in a shared world. Owners received email about significant events and could see family trees. More than 100,000 users | Closest in spirit. Email about events was the main connection with the audience. Intervening in the world was not possible |
| [Soup of Life](https://soupof.life) | 2026; running | A continuously running artificial-life simulation with "no goals, just observation": biomes, traits, a collection of organisms | The closest active competitor for observation. The demand to "watch evolution" exists right now |
| [GoL2](https://github.com/perama-v/GoL2) | Since 2022 | A shared Conway's Game of Life on StarkNet: whoever advances the world earns the right to bring a cell to life | Closest in mechanics ("contribution → targeted intervention"), but through gas and internal tokens, with no genome or evolution |
| [Electric Sheep](https://electricsheep.org/) | Since 1999; running | Volunteers' computers do the rendering; viewers' votes act as selection | Computation and votes are not linked; as the audience grew, the project ran into server costs |
| [Network Tierra](https://tomray.me/tierra/netfaq.html) | About 1995–2000; ended | Digital organisms lived on volunteers' idle computers | Volunteers provided computing power but had no influence over the world |
| Creatures Docking Station, .NET Terrarium, Darwinbots Internet Mode | About 2001–2010 | Exchanging organisms between players' worlds | The networked modes depended on the developer's server — hence the world handover plan (§26) |
| r/place, The Button, Twitch Plays Pokémon, [Eterna](https://en.wikipedia.org/wiki/EteRNA) | 2014–2023; Eterna since 2010 | Scarce collective actions, cooldowns, shared choices | Scripts are inevitable; status without money works. Eterna closes the loop "vote → experiment → result" |
| The Sapling, RimWorld Twitch Toolkit | 2020s | Viewers influence the world by voting or by buying events | A 24/7 broadcast is fragile; stronger interventions should cost more |
| [Screeps: World](https://docs.screeps.com/api/) | Since 2016; running | A persistent shared world of players' programs | Sustained for years by subscriptions; computation only buys cosmetics |
| Gridcoin, Qubic, CryptoKitties, Axie Infinity | Since 2013 | Computation or genomes as an asset | A token distorts incentives: speculation, bots, capture of computing power. This confirms the "no token" decision |
| The Bibites, Species ALRE, Thrive, ALIEN, Avida-ED and others | Since 1994 | Evolution on your own computer | The demand for watching evolution exists, but there is no shared world |

**Nobody combines "CPU work buys interventions in an evolving world".** Only separate pieces of the mechanic were found:
- GoL2: a contribution grants the right to intervene, but through a blockchain;
- Gridcoin: computation gives voting weight, but with a token;
- Screeps: computation turns only into cosmetics;
- Coinhive (2017–2019): browser mining in exchange for perks on websites; it became a tool for hidden mining and shut down.

**What is new in Protogaea:**
- sparks accumulate on public wishes, the price behaves like difficulty, and the number of miracles per epoch is limited;
- a seasonal geological story;
- verifiability without a blockchain: replay, Merkle trees, a CT-style spark log, drand, OpenTimestamps.

The project claims no scientific novelty in the evolution itself (§1).

**Lessons incorporated into the specification:**
1. The "While you were away" digest and the chronicle matter more than the stream (§7).
2. The world must outlive the operator's server, hence the world handover plan (§26).
3. Computing power can be bought, so concentration statistics are open and sparks cannot be transferred (§6).
4. A stronger intervention costs more, so each action type has its own price coefficient (§19).
5. Only explicit consent and no background work. This is already in §8 and §24; the history of Coinhive shows where the opposite leads.

*TechnoSphere, Soup of Life and GoL2 were verified manually. The rest is based on open sources; the reasons some projects closed are unconfirmed. Projects that live only in closed communities may be missing from this review.*
