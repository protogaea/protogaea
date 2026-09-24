# Licensing

## Today

Everything currently in this repository — the specification and the documentation — is licensed under the [Apache License 2.0](LICENSE), unless a file or directory says otherwise.

The one exception today is [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md): it is the Contributor Covenant 2.1, which is licensed under CC BY 4.0, with our contact details added.

## Planned licenses per component

These follow [spec §26](docs/spec/spec-v0.2.md#26-openness-licenses-and-language).

| Component | License | Why |
|---|---|---|
| Simulation core, `ruleset`, protocol specification, replay, watcher, spark client | Apache-2.0 | Maximum freedom for independent verification, ports and second implementations; an explicit patent grant |
| Server, web application, desktop app (which includes the viewer) | AGPL-3.0 | Anyone who runs a public copy as a service must publish their changes; this protects the future hosting of private worlds for education |
| World log, snapshots, season archives | CC0 or CC BY 4.0 — to be decided before Season 1 | Researchers and archivists can freely use and mirror the world's history |
| Root dictionary, text templates, artwork | CC BY-SA 4.0 (candidate) | Reusable with attribution |
| Name and logo | Not licensed | See the [trademark policy](TRADEMARKS.md) |

Each component will carry its own `LICENSE` file in its directory when it is added. Where a directory has no `LICENSE` file, the root Apache-2.0 license applies.

## Contributions

Before accepting any external code or text, we will choose between a Contributor License Agreement (CLA) and the Developer Certificate of Origin (DCO). Until then, external pull requests are not accepted; see [CONTRIBUTING.md](CONTRIBUTING.md).
