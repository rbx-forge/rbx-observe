# rbx-observe

Read the **public** storefront of a Roblox experience: passes, developer
products, prices, badges, icons, metadata, and how often any of it changes.

> **Status: skeleton.** The repository exists, the CI is wired, and nothing is
> implemented yet. Every subcommand currently fails saying so. Watch the
> tracker rather than the tags.

## What it is for

Market research on a category you are shipping into. What do comparable
experiences sell, at what prices, how often do they change them, what does
their badge cadence look like. All of it is information any player can see by
opening the experience's page — this tool reads it systematically instead of by
hand.

## The boundary, which does not move

**Game-level, aggregate, public data only. Never person-level.**

- No player lists, no account tracking, no per-user history, no join/leave
  telemetry, nothing keyed to a person.
- Polite rate limiting, and no "mirror the whole catalog" mode. This asks about
  a handful of experiences you name, at a pace a human could roughly sustain.

That line is what separates market research from surveillance. It is a design
constraint, not a policy waiting to be relaxed: a feature request that crosses
it is declined regardless of how it is framed. No credential of any kind is
needed to run this tool, and that is the same constraint seen from the other
side — everything it reads is public, so nothing it reads is yours to protect.

## Relationship to rbx-cli

[`rbx-cli`](https://github.com/rbx-forge/rbx-cli) reconciles **your** universe
against **your** declared configuration. It carries a minimal
`rbx shop observe` that reads a third party's storefront and can write it out
in `rbxshop.toml` format — useful, but a different product growing inside the
wrong tool.

This repository is where that idea gets to grow. When it is usable, `rbx-cli`
decides whether to keep its short version as a convenience or point at this one
([rbx-cli#48](https://github.com/rbx-forge/rbx-cli/issues/48)).

## Install

Nothing to install yet. When there is, it will be a Rokit tool like its sibling.

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
