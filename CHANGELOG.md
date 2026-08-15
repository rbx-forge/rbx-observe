# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

No release yet. Build from source.

### Added

- **`charts`**: Roblox's own discovery rankings, including `top-earning` and
  the fourteen `trending-in-<category>` sorts that are its real genre taxonomy.
  The only command that needs no id — it is where the universe ids come from.
  `--sort`, `--category`, `--limit` (rendering only; `--json` stays complete).
- **Eight read commands**: `game`, `storefront`, `badges`, `media`, `places` —
  each taking a universe id, a place id (`--place`) or a roblox.com game URL —
  plus `group` and `asset`. All of them support `--json`.
- **`places`**: every place in a universe, whether each looks published or
  internal, and the live server fleet on it (servers, players, seats, fill
  rate). The published state is an inference from the place's product id and
  says so; Roblox's own `isPlayable` needs a session.
- **`group` reports live players and unlisted games.** The catalog's CCU costs
  one batched request for up to 50 universes. Roblox also hands anonymous
  callers a group's staging and test places — those are counted in the summary
  always and named only under `--all`.
- **The age gate is reported separately from the maturity label.** They are
  independent: two experiences can both be `Minimal` while one is open to
  everyone and the other is 16+.
- **`game` reports the page in full**: description, the maturity label
  (`Minimal`, `Mild (9+)`, …) with the content descriptors Roblox actually
  found, and every place in the universe rather than only the root one. Per
  place public/private is not exposed anonymously, and the output says so
  instead of guessing.
- **`group`**: a studio, its member count and entry policy, and every public
  game it publishes, most-visited first. Its owner, its shout and its
  membership roster are in the payloads and are deliberately not read.
- **Output carries ids, not URLs.** A URL on every row was long, repetitive
  and not even an image link: the CDN URL cannot be built from an asset id, so
  what was printed was the API call. Listings are now one line per item and end
  with a single `rbx-observe asset <ids…>` hint.
- **`asset`**: any asset id resolved to the CDN URL that renders it, batched,
  with `--size` validated before the request. A URL is only reported for a
  `Completed` render — Roblox answers a bad id with a placeholder image and no
  error, so trusting the URL over the state would hand back a working link for
  a wrong id.
- **Asset ids everywhere they exist.** Icon (through the place asset, the only
  public route to an icon's asset id), page banner, every carousel screenshot,
  and the icon of every pass, product and badge.
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
