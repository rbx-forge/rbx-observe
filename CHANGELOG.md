# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

No release yet. Build from source.

### Added

- **Four read commands**: `game`, `storefront`, `badges`, `media`, each taking
  a universe id, a place id (`--place`) or a roblox.com game URL, and each with
  `--json`.
- **Asset ids everywhere they exist.** Icon (through the place asset, the only
  public route to an icon's asset id), page banner, every carousel screenshot,
  and the icon of every pass, product and badge — each printed with the URL
  that renders it.
- **The `AssetTypeId` guard.** `economy.roblox.com/v2/assets/{id}/details`
  answers `200` with an unrelated asset when handed a universe id instead of a
  place id. Any response that is not a Place is refused rather than reported.
- **Adaptive pacing.** One request per second per host, widening 1.5× on every
  429 up to 6s and never narrowing, because the Roblox quota is a sliding
  window rather than a flat rate. Retries on 429 and 5xx with 1s/2s/4s backoff.
- **Docs**: [docs/commands.md](docs/commands.md) and
  [docs/endpoints.md](docs/endpoints.md), the latter carrying every endpoint's
  pagination idiom, its traps, and the measurements behind the pacing.
- Repository skeleton: CI (fmt, clippy `-D warnings`, test, doc), contribution
  and security files, issue templates, DCO sign-off from the first commit.
