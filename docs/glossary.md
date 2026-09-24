# Glossary and world vocabulary

The canonical glossary is [Appendix A of the specification](spec/spec-v0.2.md#a-glossary). This page is a usage guide: which words the interface and the documentation use, and which ones they avoid.

## World vocabulary

These are the words people see. Code and protocol fields keep their technical names.

| Term | Meaning | Russian | In code |
|---|---|---|---|
| Protogaea | The world and the project; "proto-Earth" | Протогея | — |
| Naturalist | A person with a key: watches, proposes, kindles sparks | натуралист | key holder |
| Kindle sparks | Let your processor compute proof of work for a wish | разжигать искры | PoW computation |
| Spark | One accepted proof of work, bound to one wish; not a currency | искра | `spark` (share) |
| Wish | A public proposal for one intervention; collects sparks from anyone | замысел | `proposal` |
| Miracle | A wish that reached its price and was applied to the world | чудо | `miracle` |
| Price of a miracle | How much work a wish needs before it executes | цена чуда | `price` |
| Clade | A branch of the family tree; the main unit people follow | клада | `clade_id` |
| Ringing | Marking an organism to follow it; free and does not affect the world | кольцевание | `ring` |
| Museum | The record of extinct named clades; the source for revival | музей | museum |
| Spore bank | Original genomes kept for natural revival and for `revive` | банк спор | spore bank |
| While you were away | The personal digest on return | Пока вас не было | digest |
| Chronicle | The daily story of the world, built from events | летопись | chronicle |
| Season | A chapter of the world with fixed rules | сезон | `ruleset_id` |
| Time of year | Spring, summer, autumn or winter inside the world | время года | `season` in `season_mult` |
| The Breaking of Pangea | Season 1: the supercontinent splits apart | Раскол Пангеи | — |

## Words we avoid

| Avoid | Why | Use instead |
|---|---|---|
| mining, miner | Triggers antivirus software, conflicts with app store rules, evokes crypto | kindle sparks; spark client |
| token, coin, reward, earn | There is nothing to earn or own | describe participation: "took part in 5 miracles" |
| win, winner | Miracles are not prizes | "the wish was fulfilled" |
| lottery, ticket | The lottery was replaced by wishes | wish, spark |
| species (for a clade) | We make no claim of biological speciation | clade, branch; "new species" only as a conventional name |

## Notes

- English terms are candidates until native speakers review them before stage B ([spec §2](spec/spec-v0.2.md#2-the-language-of-the-world)). The fallback for "wish" is "intent".
- Clade names are Latin binomials, such as *Cursor deserti*, and are never translated.
- "Season" always means a game season. Inside the world, say "time of year".
