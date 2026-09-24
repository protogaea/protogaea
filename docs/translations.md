# Translations

## Languages

- **English is canonical** for code, documentation and the specification. If a translation differs from the English text, the English text prevails.
- **Russian** is the second language from day one, in the interface and for key documents.

## Current status

| Document | English | Russian |
|---|---|---|
| README | [README.md](../README.md) | [README.ru.md](../README.ru.md) |
| Specification v0.2 | [spec-v0.2.md](spec/spec-v0.2.md) | [spec-v0.2.ru.md](spec/translations/ru/spec-v0.2.ru.md) |
| Specification v0.1 (archived) | — | [spec-v0.1.ru.md](spec/archive/spec-v0.1.ru.md) — the original draft, never translated |
| Other documents | Yes | Not yet |

## Rules for document translations

- Translations live next to the original: `README.<lang>.md` in the root, and `docs/spec/translations/<lang>/` for the specification.
- Every translation starts with a note that it follows the canonical English version.
- Section numbering, tables and anchors match the original exactly, so the versions can be compared line by line.
- Identifiers, formulas, code, file names and protocol strings are never translated.
- World vocabulary follows the [glossary](glossary.md). Clade names (Latin binomials) are never translated.
- Translations are updated when a version is released, not on every edit. Until then, a translation may lag behind, and its header says which version it follows.

## Interface localization

- All interface strings live in resource files, never in code. This includes templates for events, the chronicle and notifications.
- English is the default; Russian ships from the first release.
- Plural forms and word order are handled by a message format that supports them (candidate: ICU MessageFormat), because Russian has three plural forms.
- Generated names of clades and places come from language-neutral roots, so they need no translation.

## Adding a language

1. Open an issue proposing the language.
2. Translate the world vocabulary first, and have a native speaker review it.
3. Then the interface strings, then the README, then the specification.
