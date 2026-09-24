# 0010. English first

- **Status:** Accepted
- **Date:** 2026-09-25
- **Specification:** §26

## Context

The project began in Russian: its author and the first drafts of the specification are Russian-speaking. The intended audience — artificial-life enthusiasts, researchers, teachers and open-source developers — is worldwide and mostly reads English. Identifiers, commit history and issue discussions are expensive to translate later. A Russian-speaking community is still expected and welcome.

## Decision

- **English is canonical** for code, identifiers, comments, commit messages, issues, pull requests, the API, the specifications and the README.
- **Documentation translations** are maintained alongside the originals and updated at releases. If a translation differs from the English text, the English text prevails. The specification v0.2 was translated on 2026-09-25, and the Russian text became a translation.
- **The interface is localized from day one:** English by default, Russian as the second language from the first release. All strings live in resource files.
- **Community:** English is the main language; a Russian-speaking channel runs separately.

## Consequences

- One source of truth, readable by the widest audience.
- Translations may lag behind between releases; their headers say which version they follow.
- Interface strings need a message format that supports Russian plural forms (candidate: ICU MessageFormat).

## Alternatives considered

- **Russian only, translation later** — translating identifiers and history later is expensive, and it would shut out most of the audience.
- **Two canonical languages** — two sources of truth that would inevitably diverge.
