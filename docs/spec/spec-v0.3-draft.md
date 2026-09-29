# Specification v0.3 draft: patrons of clades

**Status:** draft, 2026-09-29. The first decisions are made (§13), and stages 1a, 1b and 1c have been checked in the balance harness (§11). Not adopted: Season 1 still runs on v0.2. [Russian translation](translations/ru/spec-v0.3-draft.ru.md).
**Relation to v0.2:** this is a document of changes. Everything not mentioned here stays as in [v0.2](spec-v0.2.md). Section numbers of v0.2 are given in parentheses: (§6) is section 6 of specification v0.2.
**Terms:** a "clade" is a branch of the lineage tree, what one would call a species in conversation (§12). "Patron" is a new term of this draft.
**Numbers marked "candidate"** are chosen in the balance harness (§28) and frozen before a season.

## 0. What changes

| Area | v0.2 | v0.3 (draft) | Why |
|---|---|---|---|
| Player fantasy | A naturalist with a small budget of miracles | **Patron of clades**: chooses their clades, puts sparks into them, follows them and comes to the rescue | Attachment to "my own" is stronger than to a region of the map; a reason to come back every day |
| What sparks buy | Environmental miracles: weather, migration, revival | **Help for one's clade**: relief (temporary), gifts (inherited genes), hybridization; **harm to rivals**; migration and revival | More influence on the game, and the influence is personal |
| Weather | A miracle bought with sparks | **A random natural event**, like wildfire and plague | One more factor that nobody chooses |
| Price | One epoch price × an action multiplier | Plus a **clade share multiplier**: helping the leader is expensive, helping an outsider cheap; harm the other way round | Without it the largest contributor crushes the world with one clade, and a crowd hounds a small one |
| Genome | 6 traits, budget 24 | 8 traits, budget 32: **size** and **longevity** added; plus gift slots outside the budget | More trade-offs, more ways to succeed |
| Diversity | 6 founders × 50, three niches | 10 founders × 40, **new niches**: scavengers, shore dwellers, burrowers | Diversity rests on niches, not on the number of starting species |
| Season outcome | A hall of fame of events | Plus a **table of clades** with their patrons | "A battle of species for survival and dominance" gets a score |

What does **not** change: no token, no money for influence, no spark balance. Sparks are bound to a public wish and a key; they cannot be saved, transferred or sold (§1, §6). The whole world can still be verified by replay.

# Part I. The game

## 1. Player fantasy

A player comes to Protogaea, lights sparks on their processor and **chooses their clades**, one or several. From then on they are a patron: they put sparks in so that their clades survive more easily, gain new abilities and reach new lands. They follow them: who is pressing them, where they go hungry, where their descendants went. The outcome is not promised: help changes the odds, and evolution decides.

**The main criterion of the project stays** (§1): take the sparks away, leave only the stories of the ecosystem, and people still want to open the world tomorrow. Helping a clade is a way to weave the player into those stories, not to replace them.

**A change of tone.** v0.2 promised "a calm naturalist's journal". v0.3 adds rivalry: clades compete for survival and dominance, with teams of patrons behind them. The interface keeps both notes: the observation journal and the table of clades.

## 2. Roles and the game loop

- **Viewers** and **explorers**: as in v0.2 (§3).
- **Patrons** (formerly "naturalists who light sparks") keep a list of "my clades" and put sparks into them.

**The patron's loop:** arrive → light sparks → choose clades → invest → follow → rescue → season outcome → come back.

1. **Choice.** The player marks clades on the map or in the tree as "my clade". Hints while choosing: which clades are young, which are threatened, which have few patrons.
2. **Investment.** A wish to help one's clade (§4), or support for someone else's wish for the same clade.
3. **Following.** A "my clades" panel, notifications in the bot, the "While you were away" digest (§8).
4. **Rescue.** Hunters press the clade: shelter; hunger: forage; a bridge is closing: migration or the gift of swimming.
5. **Rivalry.** A neighboring clade takes over the feeding grounds: blight or sickness against it (§4.4); rivals weakened yours: shelter and cure remove their harm.
6. **Outcome.** The table of clades at the end of the season (§9); the player sees how their clades lived through the season.

## 3. My clades

- **A list of clades** per key: up to 5 clades (decided; can be revisited from experience). The choice is not exclusive: any number of patrons can lead one clade, and they become a team on their own.
- **Descendants stay on the list.** When a clade splits (§12), the daughter clades are added to the list automatically: "my clade and its branches". The player can remove the ones they do not want.
- **The list does not affect the world** and is not part of the state, like ringing (§5). The server keeps it, signed by the key.
- **A clade's patrons** are the keys that put sparks into wishes about it. This is derived from the spark log and can be verified. A clade's card shows its patrons and their shares, like the contribution shares of a wish (§6).

## 4. Helping a clade

Help consists of new wish types. The wish mechanics do not change (§6, §19): a wish is public, work accumulates, it is carried out when it reaches its price, and at most 3 miracles happen an epoch. A help wish always names a clade and a chosen 5 × 5 area where at least 10 of its organisms live (as for `migrate`).

Migration and revival stay. **Weather is no longer a miracle** and becomes a random natural event (§4.5).

### 4.1 Relief: temporary and only for one's own

A new ability: the effect applies **only to organisms of the chosen clade** in a 7 × 7 area around the center (decided). Organisms of other clades do not get it, even standing next to them. The map draws a relief area in the clade's color.

| Action | Effect (candidate) | Duration | Check |
|---|---|---|---|
| `shelter` | +10 defense for the clade's organisms (like biome shelter, §11.3) | 36 ticks (3 epochs) | The area does not overlap an active shelter of the same clade |
| `forage` | The clade's organisms' metabolism −20% | 36 ticks | The same |
| `cure` | The clade's organisms do not die of plague | 72 ticks (6 epochs) | The clade has not been cured in the last 36 epochs |

Relief is not inherited: an organism born under shelter has it while it stays in the area and the effect lasts, but does not pass it on.

### 4.2 Gifts: inherited abilities

A gift is a **gene outside the trait budget**. When the wish is carried out, up to 10 of the youngest organisms of the clade in the 5 × 5 area receive the gift (they live longer, so they have more time to pass it on). From then on it is inherited like the other genes:
- at birth the gift is lost with probability `gift_loss_rate` (0.5%; at 2% and five gifted organisms most gifts were lost to drift within 3 days);
- an organism holds at most 2 gifts;
- every gift has an **upkeep**: energy per tick, like traits (§11.2). A gift that does not pay for itself is washed out by selection.

Gifts do not count toward the distance between genomes, like behavior genes (§12): a gifted clade does not split into a new one, even when the gift has spread to all its organisms (decided). The gift is shown on the clade's card.

| Gift | What it gives (candidate) | Upkeep | The story it is for |
|---|---|---|---|
| `swim` | Crosses shallows without the double step cost and without exhaustion; through deep water one cell a tick, at an energy cost | Medium | Crossing a strait after a bridge has closed |
| `venom` | A predator that kills a venomous organism loses 30% of its energy; +6 defense | Medium | Defense for herbivores without armor |
| `camo` | +6 defense (in the harness, instead of "a predator sees one cell less") | Medium | Living next to hunters |
| `keen` | Sight radius +1 and +4 defense (sees a threat earlier) | Minimal | Seeing food and threats earlier |
| `hardy` | Half the metabolism penalty outside the preferred biome | Low | Spreading into other biomes |
| `scavenge` | Completes a bite with detritus, worth a third of food | High | A new niche: feeding on graveyards |

**Why inherited and not buffs.** The contribution works through evolution. A successful gift spreads to descendants and can outlive the clade itself, turning up in its branches on another continent. An unsuccessful one disappears, and that is a story too. Buffs without inheritance turn the game into grinding numbers.

**Rejected option:** "+1 to a trait above the budget". It breaks the genome's main trade-off (one trait grows only at the expense of another) and within a few gifts yields the "perfect build" that §5 of v0.2 protects us from.

### 4.3 Reinforcement

- `migrate` and `revive`: as in v0.2. A clade's patron can revive its extinct branches from the museum.
- `hybrid`: a new miracle, §5.

### 4.4 Harm to rivals

The mirror of relief: the effect applies **only to organisms of another clade** in a 7 × 7 area where at least 10 of its organisms live. Harm never kills directly: it changes energy, defense and odds, and ecology decides the outcome.

| Action | Effect (candidate) | Duration | Removed by |
|---|---|---|---|
| `blight` | The clade's organisms' metabolism +20% | 36 ticks (3 epochs) | `forage` for the same clade in this area |
| `expose` | The clade's organisms' defense −10 (they lose biome shelter) | 36 ticks | `shelter` |
| `sickness` | The clade's organisms' chance of dying of plague ×3, but no more than 5% of the area's organisms a tick | 36 ticks | `cure` |

**Protections against hounding:**
- **The price of harm mirrors the share rule** (§6): harming the leader is cheap, an outsider expensive.
- **Small clades are untouchable:** harm cannot target a clade of fewer than 20 organisms or less than 3% of the population (in the harness, at 2% 12% more clades went extinct than without players). Harm cannot finish a clade off: ecology has the last word.
- **Respite:** after harm, a clade is protected from new harm of any kind for 24 epochs.
- **Relief beats harm of the same kind:** an active shelter cancels expose, forage cancels blight, cure cancels sickness. Patrons can fight back.
- **Harm is public,** like any wish: anyone can see who is behind it and with what shares.
- **Harm does not take gifts away.** A clade's legacy is protected.

### 4.5 Weather as a natural event

Weather is no longer bought. Rain and drought (the effects of the former `weather`: food growth ×1.5 or ×0.5, moisture ±30, 36 ticks, 7 × 7) fall by themselves at an epoch boundary, like wildfires and plague (§10 of v0.2), by counter-based randomness from the beacon. Candidate: 3% an epoch for each of 8 large regions of the map, at most once in 12 epochs per region; the harness chooses the frequencies. This is one more factor that nobody chooses, the operator included.

## 5. Hybridization

| Parameter | Candidate |
|---|---|
| Parents | Two living clades, 5+ organisms of each in one 5 × 5 area |
| Compatibility | The distance between the clades' reference genomes is 2 to 8 steps: closer, and it is the same clade; farther, and it is sterile |
| Result | 8 hybrids in a 3 × 3 area (4 in the first draft; see §11): each trait is taken from one of the parents by counter-based randomness (§14), then the budget is brought to 32 by mutation steps under the rules of §11.1 |
| Gifts | Each of the parents' gifts passes to a hybrid with probability 50%, at most 2 |
| Clade | The hybrids found a new clade with two parents |
| Hybrid vigor | For 6 world hours the hybrid clade has `forage` (−20% metabolism) around the crossing: without it 8 organisms at the world's equilibrium die out by chance (§11) |
| Limit | One pair of clades hybridizes at most once a world day; price multiplier 250 |

**Consequence for the interface:** a clade can have two parents, and the clade tree becomes a network. The tree, the Muller plot, the museum and the cards have to learn to show two parents.

## 6. The price of help and harm; protection from hegemony and hounding

**The problem.** Patrons flock to the leader, the leader gets more help and grows stronger. A large contributor can choose a clade and crush the whole world. The world becomes a monoculture and boring for everyone, that contributor included.

**The share rule.** The price of a help wish is multiplied by the clade's share multiplier `s` (the share of its organisms among the living when the window opens):

`price = P_E × price_mult[action] / 100 × share_mult(s) / 100`

`share_mult(s) = max(75, s² / 100)` with the share `s` in permille (chosen in the harness, §11): with the earlier ×2.5 at 20%, help for the leader changed the leader too rarely.

| Clade share | Price multiplier |
|---|---|
| up to 8.7% | ×0.75 (a discount for outsiders) |
| 10% | ×1 |
| 20% | ×4 |
| 30% | ×9 |
| 40% | ×16 |
| 50% and above | help unavailable, except `cure` |

All values are integers: the share is stored in permille and the power is computed exactly.

**The share rule for harm is the mirror:** harming the leader is cheap, an outsider expensive, small clades untouchable.

| Target clade's share | Harm price multiplier |
|---|---|
| under 3% or under 20 organisms | harm unavailable |
| 3–5% | ×4 |
| 5–10% | ×2 |
| 10–20% | ×1 |
| 20% and above | ×0.75 |

Harming the leader is cheaper than helping it: the world pulls itself toward balance, like "plague strikes the winner" in v0.2.

**Other limits:**
- a clade gets at most one gift in 36 epochs and at most one relief of each kind at a time;
- a gift has an upkeep (§4.2), so even a free gift is not always worth having;
- the v0.2 mechanisms stay: plague strikes the winner, the grazer–armored–hunter cycle, limits and cooldowns.

**The honest limitation stays** (§6): more computing power means more influence. The share rule does not make people equal; it keeps influence from turning the world into one clade. The concentration of contributions is still shown openly: a clade's card shows the shares of its top 1, top 3 and top 10 patrons.

**Harm to rivals exists** (decided, §4.4), with protections against hounding: small clades are untouchable, a 24-epoch respite, relief removes harm of the same kind, harm does not kill directly. Rivalry also goes through ecology: migrating hunters to the rivals.

## 7. The genome: new traits and niches

### 7.1 Traits

The budget grows from 24 over 6 traits to **32 over 8 traits**, each 0–8. The new traits:

| Trait | Phenotype | Trade-off |
|---|---|---|
| `S` size | Energy capacity ×(0.70 + 0.11·S); +S to attack and defense; upkeep +6 a point; reproduction threshold +300 and offspring energy +150 a point | A large organism is strong and hardy, but eats a lot and breeds less often |
| `L` longevity | Senescence and maximum age ×(0.80 + 0.07·L); upkeep +8 a point | Lives long, but every point is taken from fertility or strength |

The trait costs were chosen in the harness (§11): at 4 a point for longevity every lineage went to longevity 7–8, at 12 to 0; at 4 a point for size giants took over the world. A grid of 9 cost pairs showed the middle: longevity 8 a point, size 6. With 8 traits a clade splits off at a distance of 4 steps (not 3): otherwise clades split too often.

"Strength, health, stamina" from the discussion map onto existing and new traits: strength is hunting and size, health is defense and size, stamina is movement, metabolism and longevity. No separate "hit points": death in Protogaea is hunger, a predator, old age and plague, not damage to a bar.

### 7.2 Niches and starting diversity

Today the world has in effect three niches: grazers, armored and hunters (§11.3). Twenty starting species would collapse into these three within a few days. So new niches come together with the rules that hold them:

| Niche | Rule | Who lives there |
|---|---|---|
| Scavengers | Detritus can be eaten: the `scavenge` gift (a third of the value of food) or innately at `G` ≥ 6 and `H` ≥ 3 (half the value of food) | They live on the graveyards behind hunters |
| Shore dwellers | Algae grow in the shallows (regrowth 35, capacity 120), and only organisms with the `swim` gift eat them | The seasonal plot of the rift |
| Burrowers | An organism with `M` ≤ 3 and `D` ≥ 5 hides when a hunter of another clade is in a neighboring cell: it skips the tick, +25 defense | Survival under predator pressure |

**The starting set:** 10 founders × 40 organisms instead of 6 × 50: the six earlier ones with size and longevity, a scavenger, a shore swimmer (born with the `swim` gift), a burrower and a giant. The map generator's requirement stays: every future continent has founders.

A niche is accepted only if the harness shows that it holds on at least half of the seeds.

# Part II. The interface and the season outcome

## 8. What a patron sees

- **A "my clades" panel:** population and its trend, range, threats (predators nearby, hunger, a bridge closing), active reliefs, gifts and how far they have spread among descendants.
- **A clade's card:** patrons and shares, the history of help, gifts among its organisms, parents (two for a hybrid).
- **A "what if" forecast:** an ensemble of runs "with the help and without" before investing, in the browser, as the time machine already works (§7). For example: "with shelter the clade survives the raid with 64% probability, without it 21%".
- **Notifications in the bot** about one's clades: a threat, a gift has spread, the clade has split, the clade went extinct, a branch came back.
- **The attention map** (§6) shows where help goes.

## 9. Season outcome: the table of clades

"A battle of species for survival and dominance" gets a score. At the end of the season a table of clades is published:

| Title | How it is counted |
|---|---|
| Longest-lived | The clade that lived longest (with its branches) |
| Sovereign | The longest time as the world's largest clade |
| Conqueror | Lived on the most continents at once |
| Comeback | The largest growth after a minimum |
| Legacy | The gift that spread the widest |
| People's choice | The clade with the most patrons |

Next to a clade are its patrons with their shares. A patron's profile: "led 4 clades, 2 lived to the finale, 3 gifts". No points for contribution: the reward is the clade's story, not the amount of work spent.

# Part III. Rules and protocol

## 10. Changes to the protocol and the state

- **New actions in a wish** (§17): `shelter`, `forage`, `cure`, `gift` (with a gift code), `hybrid`, `blight`, `expose`, `sickness`. Each names a `clade_id` and an area; `hybrid` names two clades. `weather` leaves the wishes: with patrons a `weather` wish is refused. Codes 3–10 and their canonical encoding are in [`docs/protocol.md`](../protocol.md) (Proposed), in the protocol crate, the server, the watcher and the spark client.
- **Already in the core** (behind the optional ruleset sections `patrons` and `traits8`, and `Ruleset::v03()`): relief, harm, gifts, hybridization, natural weather, size, longevity and the niches. Without these sections the ruleset id and the state roots are unchanged (checked byte for byte).
- **Natural weather** (§4.5) falls by the beacon and is part of the epoch's events, like wildfires and plague.
- **Price** (§19): the share multiplier is computed from the state the miracle applies to (the end of epoch E−1), so it is deterministic and a watcher can replay it. The server and the watcher price wishes this way; a wish the share rule closes waits and is not a candidate that epoch. `GET /v0/ledger?clade=ID` gives a clade's multipliers.
- **State** (§15): gift slots in an organism's genome; active reliefs and harms with their clade and area; clades' respites after harm; a clade's second parent; gift and hybridization cooldowns. All of it enters `state_root`.
- **Ruleset:** a new `ruleset_id`. The live test world starts anew; there has been no public season yet, so the rule "a season's rules never change quietly" is not broken.
- **The watcher and the time machine** replay the new actions just as they replay today's miracles.
- **The "my clades" list** is neither in the state nor in the spark log: it is a setting signed by the key.

## 11. The balance harness: proving that the world holds

Before any code in the live world, the mechanics are checked in the harness (§28). The harness gets **patron bots** with different strategies:

| Strategy | Behavior |
|---|---|
| Leader | All bots help the largest clade |
| Weak | They help the smallest clades |
| Random | They choose clades and help at random |
| Whale | One bot with 50% of all work leads one clade the whole season |
| Harass | Bots harm the smallest clade that can be harmed |
| War | Two camps each lead a clade, help their own and harm the other |
| Mixed | 70% random, 20% leader, 10% whale |

**Results (stage 1a, 2026-09-27, 16 seeds × 14 days):** the world holds under every strategy; the leader changes at least once in 3 days under any strategy; named clades go extinct no more often than without players (296–349 against 350 ± 15); a long lead of one lineage is natural (without players, the lineage leading after the first day ends larger than the second on 15 seeds of 16); 20–34% of gifts are still held 3 days later, keen sight 16% (raised to +4 defense). Details are in the [harness README](../../harness/README.md#findings-spec-v03-draft-patrons-of-clades).

**Results (stage 1b, 2026-09-28, the same 16 seeds × 14 days, the full v0.3 rules):** the world never went extinct; 6+ clades of 20+ organisms 90–98% of the time after day three (98% without players); no clade holds more than 60% for longer than 1.2 days; mean size 1.7–2.5 and longevity 3.3–5.5; at the end 150–450 swimmers and 24–68 giants, 4–7 burrowers and 2–33 scavengers (small niches); 352–432 named clades went extinct (about 350 in v0.2). The swimming gift is kept in 84% of cases: the algae make it strong ([details](../../harness/README.md#findings-spec-v03-draft-size-longevity-and-new-niches)).

**Results (stage 1c, 2026-09-29, 16 seeds × 14 days, the full v0.3 rules with hybridization):** a hybrid clade lives about as long as a young clade of its size founded by mutation: with 4 hybrids 5% are alive with their descendants a day later and 2% three days later (young clades of 4+: 7% and 3%), with 8 hybrids 9% and 3% (young clades of 8+: 10% and 4%). Survival depends on the size of the start, so the crossing places 8. The hybrids themselves are fit (their founders breed more often and fall to hunters less than those of young mutation clades), but in a world at equilibrium a group of eight dies out by chance. Hybrid vigor, `forage` for 6 hours, raises survival to 27% a day later and 9% three days later, and 64% of hybrid families reach 20 living. The world holds under every strategy: no extinction, 6+ clades of 20+ 96–99% of the time, no clade over 60% for longer than 0.6 days, 334–456 named clades extinct; under the strategies that help most the §28 checks pass in 69–71% of seasons against 76–80% before hybridization ([details](../../harness/README.md#findings-spec-v03-draft-hybridization)).

**Passes if:**
- the v0.2 health metrics (§28) are met under every strategy, "leader" and "whale" included;
- the whale's clade does not hold more than 60% of the population for longer than 3 days;
- under "harass" clades go extinct no more often than without players: harm by itself does not finish clades off;
- in "war" neither camp always wins: the outcome depends on ecology, not only on the amount of work;
- every gift, once given, takes hold in 20–80% of cases: always means too strong, never means useless;
- the "what if" forecast for relief shows a noticeable effect (otherwise help creates no stories, like the too-weak weather of v0.1).

## 12. Stages

1. **Harness**: done for 1a (relief, harm, gifts, natural weather, patron bots), 1b (size, longevity, niches, 10 founders) and 1c (hybridization); the costs are chosen.
2. **Specification v0.3**: a draft in English and Russian; decisions go into the decision log when it is adopted.
3. **Core, server, watcher:** the new actions, the share price, the state. In progress: the actions and the share price are in the protocol, the server, the watcher and the spark client.
4. **Viewer and bot:** "my clades", the patrons' card, the forecast, notifications.
5. **A new test world** with the v0.3 ruleset.

## 13. Decisions and open questions

**Decided (2026-09-27):**
1. **Harm to rivals exists** (§4.4), with protections against hounding.
2. **Up to 5 clades** on a patron's list; to be revisited from experience.
3. **Gifts do not split a clade:** even a gift that has spread across the whole clade does not create a new branch.
4. **Relief only for one's own:** it applies only to organisms of the chosen clade.
5. **The term is "patron".**
6. **Weather is a random natural event**, not bought (§4.5).

**Open:**
1. The numbers of all effects and prices, from the harness.
2. How many gifts an organism holds: 2 or 3?
3. Whether a separate harm against gifts is needed (for example, "weaken a clade's gift") or gifts stay untouchable, as now.
4. English terms: patron, gift, blight, expose, sickness; to be checked with native speakers.

## 14. Risks

| Risk | Consequence | Mitigation |
|---|---|---|
| Hegemony of a popular clade | A monoculture, a boring world | The share multiplier, no help at 50%+, gift upkeep, the harness with the "leader" strategy |
| A whale with great computing power | Their clade always wins | The same share rule; open patron shares; the harness with the "whale" strategy |
| Unbalanced gifts | One meta build displaces the rest | Upkeep, the limit of 2 gifts, loss at birth, the 20–80% take-hold check |
| A complex interface | A newcomer does not know what to do | A simple first step: "choose a clade, put sparks into shelter"; the "what if" forecast shows the point |
| Hounding of small clades | A crowd finishes off rare species | Clades under 3% or 20 organisms are untouchable, harm costs ×4 against small clades, a 24-epoch respite, harm does not kill directly, the harness with the "harass" strategy |
| The tone drifts to "grinding" | The value of observation is lost | Inherited gifts and an outcome through evolution; the table of clades without points for contribution |
| Amount of work | The season is delayed | Stages 1–2 first: if the harness shows the world does not hold, the mechanics change before the code |
