# rbx-observe

Read the **public** storefront of a Roblox experience: passes, developer
products, prices, badges, carousel screenshots, and the asset ids behind all of
it.

```sh
rbx-observe charts --sort top-earning
rbx-observe snapshot <universe-id> --json > 2026-08-15.json
rbx-observe game https://www.roblox.com/games/<place-id>/<name>
rbx-observe storefront <universe-id>
rbx-observe badges <universe-id> --json
rbx-observe group <group-id>
rbx-observe asset <asset-id> --size 1024x1024
```

No API key. No cookie. No account. Everything it reads is what a logged-out
visitor sees.

> **Status: early (0.2.0).** Nine commands, tested against recorded response
> shapes. Field names in `--json` can still move before 1.0.

## What it is for

Market research on a category you are shipping into. What do comparable
experiences sell, at what prices, how many badges do they hand out and how
often, does their page open on a video. All of it is public information; this
reads it systematically instead of by hand.

The asset ids are the part you cannot get by looking. `rbx-observe` resolves
the icon, the banner and every carousel screenshot down to permanent asset ids,
which is what makes a storefront comparable across weeks rather than a
screenshot you took once. `rbx-observe asset` turns any of them back into an
image.

## The boundary, which does not move

**Game-level, aggregate, public data only. Never person-level.**

- No player lists, no account tracking, no per-user history, nothing keyed to a
  person. Presence (where an account is right now), inventories, profiles, a
  person's group memberships, a group's owner and roster, and the user-keyed
  game catalog are all public, all easy, and all deliberately absent.
  [docs/endpoints.md](./docs/endpoints.md) lists each one with its reason,
  where somebody would look before adding it. Where the excluded field sits
  inside a payload the tool *does* read — a group's `owner` — it is simply not
  declared on the struct, so it never enters the process, and a test asserts
  that.
- Polite rate limiting: paced requests that widen on every 429, and no "mirror
  the whole catalog" mode. It asks about experiences you name, one at a time.
- No credential of any kind, which is the same constraint from the other side:
  everything it reads is public, so nothing it reads is yours to protect.

That line separates market research from surveillance. It is a design
constraint, not a policy waiting to be relaxed: a feature request that crosses
it is declined regardless of how it is framed.

## Install

With [Rokit](https://github.com/rojo-rbx/rokit), in your project's
`rokit.toml`:

```toml
[tools]
rbx-observe = "rbx-forge/rbx-observe@0.2.0"
```

then `rokit install`. Or add it to whatever you have open:

```sh
rokit add rbx-forge/rbx-observe            # this project
rokit add --global rbx-forge/rbx-observe   # everywhere
```

The command is `rbx-observe`, same as the repository, so no `--alias` is
needed.

Prebuilt for Linux x86_64, Windows x86_64 and macOS Apple Silicon; each
release also carries `SHA256SUMS`. From source instead, with Rust 1.88 or
newer:

```sh
git clone https://github.com/rbx-forge/rbx-observe
cd rbx-observe
cargo build --release
./target/release/rbx-observe game <universe-id>
```

## Commands

| Command | What it reports |
| --- | --- |
| `rbx-observe charts` | Roblox's own rankings — top playing, top earning, up-and-coming, and the fourteen trending-by-category sorts. The command that hands out universe ids |
| `rbx-observe snapshot <target>` | Every section above in one JSON document, timestamped — the artifact to keep and compare later |
| `rbx-observe game <target>` | Description, players, visits, favorites, votes, genre, maturity label and content descriptors, every place in the universe, icon and banner asset ids, carousel size, preview video |
| `rbx-observe storefront <target>` | Game passes and developer products: prices, ids, icon asset ids, and the low/median/high of what is actually on sale |
| `rbx-observe badges <target>` | Every badge with total awards, awards in the last day, win rate and icon asset id, most-awarded first |
| `rbx-observe media <target>` | Carousel screenshots as permanent asset ids, with the preview video if there is one |
| `rbx-observe places <target>` | Every place in the universe: published or internal, and the live server fleet on each |
| `rbx-observe group <groupId>` | A studio: members, entry policy, its catalog with live players, and how many games it has that are not publicly listed |
| `rbx-observe asset <ids…>` | Any asset id resolved to the URL that renders it, batched |

`<target>` is a universe id, a place id with `--place`, or a game URL. `--json`
on any of them prints the same data machine-readably.

Where Roblox does not tell an anonymous caller the truth outright, the tool
says which part is inferred rather than guessing silently: a place's
public/private state is inferred from its product id because `isPlayable` needs
a session, and social links are not available at all
(`/social-links/list` answers `Authentication token is missing`).

Full descriptions in [docs/commands.md](./docs/commands.md); every endpoint,
its traps and the rate-limiting model in
[docs/endpoints.md](./docs/endpoints.md).

## Three numbering spaces

Universe ids, place ids and asset ids are different numbers for different
things and none of them is recognisable by looking at it. A game URL carries a
**place** id; nearly every endpoint takes a **universe** id.

Getting it wrong does not fail loudly — `economy.roblox.com` answers `200` with
somebody else's asset when handed a universe id. So `rbx-observe` converts once
at the entry point, and refuses any place-asset response that is not actually a
Place. That guard has its own test.

## Maintenance

One maintainer, maintained on my schedule. Issues are triaged, not promised.
Security reports go through [SECURITY.md](./SECURITY.md), not the tracker.
Contribution setup is in [CONTRIBUTING.md](./CONTRIBUTING.md).

Written with AI assistance. Every line is reviewed, tested and shipped by the
maintainer, who is responsible for it.

## License

[MPL-2.0](./LICENSE). Source files modified from this project must be kept
under MPL-2.0; new files added by downstream users may be licensed
independently.

`rbx-observe` is a community tool. It is not affiliated with, endorsed by, or
sponsored by Roblox Corporation. "Roblox" is a trademark of Roblox Corporation,
used here only to say what this tool reads.
