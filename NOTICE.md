# Project origin and third-party components

Kakehashi is derived from [NexQ](https://github.com/naxhq/NexQ), originally copyright (c) 2026 Vahid Alizadeh and contributors, under the MIT License. The upstream copyright and license are preserved in [LICENSE](LICENSE).

This repository starts with an independent Git history containing the Kakehashi source snapshot. It is not maintained by NexQ. The inherited version number and [CHANGELOG.md](CHANGELOG.md) describe upstream history; Kakehashi's current behavior and verification are recorded in [docs/PROGRESS.vi.md](docs/PROGRESS.vi.md). Selected upstream changes are documented in [the upstream integration report](docs/reports/upstream-sync.vi.md).

Dependencies retain their respective licenses. Model weights and native runtimes are downloaded separately and are not included in this repository. Their download locations and pinned checksums are recorded in [scripts/models.lock.json](scripts/models.lock.json); see the [local model guide](docs/guides/local-models.vi.md) for links to their publishers.
