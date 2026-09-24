# Security policy

Protogaea is at the specification stage: there is no running service and no released software yet. Security still matters now, because this is when the protocol design is being fixed.

## Reporting a vulnerability

- Please **do not** open a public issue.
- Report privately through GitHub's private vulnerability reporting: the repository's **Security** tab → **Report a vulnerability**, or directly at [github.com/protogaea/protogaea/security/advisories/new](https://github.com/protogaea/protogaea/security/advisories/new).
- Include what is affected (a specification section or document now; a component later), how it could be exploited, and the impact.
- We aim to acknowledge reports within 7 days (candidate) and will keep you informed. We credit reporters unless they prefer otherwise.

Security contact: **[security contact — to be added before the repository is published]**

## In scope now: design flaws

Flaws in the [specification](docs/spec/spec-v0.2.md) that would let someone:

- influence miracle selection or the ecology seed beyond what their work entitles them to — for example by grinding, selective inclusion of sparks or beacon manipulation;
- forge or replay sparks, or claim someone else's work;
- make the simulation non-deterministic across platforms or defeat replay verification;
- overload spark intake cheaply (asymmetric denial of service);
- learn more about naturalists than the public log reveals by design;
- trick people into running computation without their consent.

## In scope later: software

Once code exists: the simulation core, the spark service and ledger, the watcher, the desktop app and the WASM spark client, the web application, and the build and release pipeline (reproducible builds, code signing).

## Out of scope

- The v0 trust boundary as documented in [spec §21](docs/spec/spec-v0.2.md#21-verifiability-and-the-trust-boundary) — for example, the operator's ability to refuse a spark is known and stated.
- Large computing power buying proportionally more influence. This is a documented property of the design ([spec §6](docs/spec/spec-v0.2.md#6-wishes-and-sparks-how-influence-is-allocated)), not a vulnerability — unless you find a way to gain more influence than the work you spent.
- Social engineering, physical attacks and spam.

## Disclosure

We follow coordinated disclosure: we fix the problem first, then publish an advisory. The rules of a running season never change silently: if a fix requires a rules change, the season is halted with a public report and continues under a new `ruleset` ([spec §25](docs/spec/spec-v0.2.md#25-operations-load-abuse)).
