# Licensing

## Today

Everything in this repository is licensed under the [Apache License 2.0](LICENSE), unless a file or directory says otherwise. That covers the specification, the documentation, the simulation core (`core/`) and the balance harness (`harness/`).

The exceptions:

- [`server/`](server/LICENSE) (the world server) and [`viewer/`](viewer/LICENSE) (the web viewer, with `viewer/wasm`, the core built for the browser) and [`spark/`](spark/LICENSE) (the spark client) are licensed under the **GNU Affero General Public License v3.0 only** (`AGPL-3.0-only`), as planned below: anyone who runs a modified copy as a public service must publish their changes.
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) is the Contributor Covenant 2.1, licensed under CC BY 4.0, with our contact details added.
- The viewer's third-party parts keep their own licenses: PixiJS (MIT), the Geist fonts (SIL Open Font License 1.1) and Phosphor icons (MIT).

## Planned licenses per component

These follow [spec §26](docs/spec/spec-v0.2.md#26-openness-licenses-and-language).

| Component | License | Why |
|---|---|---|
| Simulation core, `ruleset`, protocol specification, replay, watcher, spark client | Apache-2.0 | Maximum freedom for independent verification, ports and second implementations; an explicit patent grant |
| Server, web application, desktop app (which includes the viewer) | AGPL-3.0 | Anyone who runs a public copy as a service must publish their changes; this protects the future hosting of private worlds for education |
| World log, snapshots, season archives | CC0 or CC BY 4.0 — to be decided before Season 1 | Researchers and archivists can freely use and mirror the world's history |
| Root dictionary, text templates, artwork | CC BY-SA 4.0 (candidate) | Reusable with attribution |
| Name and logo | Not licensed | See the [trademark policy](TRADEMARKS.md) |

Components under Apache-2.0, such as `core/` and `harness/`, use the root `LICENSE` and declare `license = "Apache-2.0"` in their manifests. AGPL-3.0 components, `server/`, `viewer/` and `spark/`, carry their own `LICENSE` file and declare `AGPL-3.0-only` in their manifests. Where a directory has no `LICENSE` file, the root Apache-2.0 license applies.

## Contributions

Before accepting any external code or text, we will choose between a Contributor License Agreement (CLA) and the Developer Certificate of Origin (DCO). Until then, external pull requests are not accepted; see [CONTRIBUTING.md](CONTRIBUTING.md).
