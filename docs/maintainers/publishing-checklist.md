# Publishing checklist

The repository [github.com/danifest751/protogaea](https://github.com/danifest751/protogaea) has been public since 2026-09-24, so anything pushed to it is published immediately. This list tracks what remains; it comes from [spec §26](../spec/spec-v0.2.md#26-openness-licenses-and-language) and [§31](../spec/spec-v0.2.md#31-decisions-to-make-before-writing-code).

## Name and namespaces

- [ ] A lawyer checks "Protogaea" in classes 9, 41 and 42 (and 28, if there will be merchandise) in the US, the EU, the UK and Russia.
- [ ] Register the domain `protogaea.com`, and ideally `.org`, `.game` and `.world`.
- [ ] Register common misspellings as budget allows (for example `protogea`, `protogaia`).
- [x] Create the public repository: [danifest751/protogaea](https://github.com/danifest751/protogaea) (2026-09-24).
- [ ] Reserve the GitHub organization name `protogaea`. The repository can be transferred to it later; GitHub redirects the old address.
- [ ] Reserve package names: `protogaea` on crates.io and npm.

## Placeholders to fill in

- [ ] The conduct contact in [CODE_OF_CONDUCT.md](../../CODE_OF_CONDUCT.md).
- [ ] The security contact in [SECURITY.md](../../SECURITY.md).
- [ ] The contact in [TRADEMARKS.md](../../TRADEMARKS.md).
- [x] The repository URL in [.github/ISSUE_TEMPLATE/config.yml](../../.github/ISSUE_TEMPLATE/config.yml) points to `danifest751/protogaea`. Update it if the repository is transferred.

## Policies

- [ ] A lawyer reviews the trademark policy.
- [ ] Approve the wording of the world handover commitment (spec §26).
- [ ] Choose a CLA or the DCO before accepting any external contribution.
- [ ] Decide the data license for world logs: CC0 or CC BY 4.0 (needed before Season 1, not before publishing).

## Repository settings

- [x] The About description is set (see below).
- [x] Add the topics (see below).
- [ ] Enable private vulnerability reporting: Settings → Code security → Private vulnerability reporting.
- [ ] Protect the `main` branch.
- [ ] Decide whether to enable GitHub Discussions.
- [ ] Add a social preview image.

## Contents

- [ ] No secrets anywhere in the Git history.
- [ ] All links in the documentation work.
- [ ] The README states the status honestly: pre-alpha, specification stage.

## GitHub "About"

**Description** (the limit is 350 characters):

> A persistent digital evolution world you can watch — and, rarely, touch. Organisms evolve on their own; naturalists pool CPU proof-of-work into public wishes that become small, bounded miracles. No token. Verifiable by anyone.

**Short variant**, for places with little room:

> A persistent digital evolution world you can watch — and, rarely, touch. No token, verifiable by anyone.

**Topics:**

`artificial-life` `alife` `digital-evolution` `evolution-simulator` `ecosystem-simulation` `deterministic-simulation` `simulation-game` `proof-of-work` `verifiable-computing` `rust` `webassembly` `open-source`

**Website:** none until the domain is registered.
