# Contributing to Protogaea

Thank you for your interest. Protogaea is at the **specification stage** (pre-alpha): the design is written down, but there is no code yet.

## What helps most right now

- **Feedback on the specification.** Anything unclear, contradictory, underspecified or wrong in [specification v0.2](docs/spec/spec-v0.2.md). Use the "Spec feedback" issue template and cite the section, for example §19.
- **Ideas.** Mechanics, visualizations, stories you would like to see in the world. Use the "Idea" issue template.
- **Prior art.** Projects we should learn from and that are missing from [Appendix E of the spec](docs/spec/spec-v0.2.md#e-comparable-projects-and-lessons).
- **Native-speaker review** of the world vocabulary (spark, wish, miracle, naturalist) in [spec §2](docs/spec/spec-v0.2.md#2-the-language-of-the-world).
- **Questions.** If something in the documentation confused you, that is a documentation bug — please tell us.

## Pull requests

We are **not accepting external pull requests yet**. The design is still moving, and we have not yet chosen between a Contributor License Agreement (CLA) and the Developer Certificate of Origin (DCO), which has to be decided before outside code or text can be merged ([spec §26](docs/spec/spec-v0.2.md#26-openness-licenses-and-language)).

Please open an issue instead. If a change is agreed on, a maintainer will make it. This section will describe the pull request process once contributions open.

## How decisions are made

- The specification is the source of truth. Changes to it are discussed in issues and recorded as [decision records](docs/decisions/README.md).
- Numbers marked "candidate" are expected to change once the balance harness exists (stage A of the [roadmap](docs/roadmap.md)).
- Once a season starts, its rules never change quietly. Rule changes go into a new `ruleset` for a new season.

## Language

- English is the canonical language for code, issues, pull requests, commit messages and documentation.
- If you are more comfortable writing in Russian, that is fine — a maintainer will translate the essentials.
- The README and the specification have Russian translations; see [translations](docs/translations.md).

## Documentation style

- Plain, precise English. Short sentences, one idea per paragraph.
- Use the world vocabulary from the [glossary](docs/glossary.md): sparks, wishes, miracles, naturalists. Never use "mining", "miner", "token", "reward" or "earn" in user-facing text.
- Mark numbers that may change as "candidate" and open questions as "TBD".
- Refer to specification sections as §N and link to them.
- Keep one source of truth: link to the specification instead of copying it.

## Code of Conduct

Everyone taking part in the project is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Security

Please do not report security problems in public issues. See [SECURITY.md](SECURITY.md).
