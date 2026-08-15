# Commands

Four commands, one shape:

```sh
rbx-observe <command> <target> [--place] [--json]
```

`<target>` is a universe id, a place id with `--place`, or a roblox.com game
URL (which carries a place id, so `--place` is redundant there). `--json`
prints the same data as a machine-readable document instead of the human
rendering.

Nothing here writes anything, and no command takes a credential.

## `game`

Everything the experience's page says about itself.

```sh
rbx-observe game 1111111111111
rbx-observe game https://www.roblox.com/games/2222222222222221/Sandbox Frontier
```

- **Audience** — players right now, lifetime visits, favorites, up/down votes
  with an approval percentage.
- **Shape** — max players, Roblox's own genre taxonomy (`genre_l1 / genre_l2`,
  not the legacy `genre` field that says "All" almost everywhere), creation and
  last-update dates, paid-access price if there is one.
- **Assets** — the icon's asset id, the banner's asset id, the carousel size,
  and whether the page opens on a video.

Five endpoints across four hosts. The icon asset id needs a sixth call to
`economy.roblox.com`; if that one fails the rest still prints, with the icon
line saying the id is unavailable.

## `storefront`

What the experience sells.

```sh
rbx-observe storefront 1111111111111 --json
```

Game passes and developer products, each with price, id, creation date and icon
asset id, then a summary: how many of each are on sale, and the low / median /
high price.

The summary counts **for-sale items only**. An off-sale pass at R$ 1 000 is a
leftover, and letting it set the maximum makes the whole row lie. `free` and
`off sale` stay distinct for the same reason.

Both catalogs are fully paginated, and the two endpoints paginate differently —
see [endpoints.md](./endpoints.md).

## `badges`

What the experience rewards, and how often.

```sh
rbx-observe badges 1111111111111
```

Every badge with its total awards, awards in the last 24 hours, win rate, icon
asset id and creation date, ordered most-awarded first — on a game with forty
badges the tail is onboarding noise and the head is what players actually
reach. Disabled badges are listed and marked rather than hidden.

The summary totals awards across all badges. `awarded_past_day` is the one
number that moves daily, which makes it the one worth trending if you record
these.

## `media`

The game page carousel, and the command that exists purely for asset ids.

```sh
rbx-observe media 1111111111111 --json
```

Each screenshot's permanent asset id with its render URL, alt text when the
developer set one, and the preview video if there is one. Video entries are
excluded from the screenshot list: they carry a poster-frame `imageId` that
would otherwise inflate every carousel with a video by one.

## Exit codes

`0` on success, `1` on any error. There is no drift concept here — nothing is
being reconciled, so there is no third state to report.

## `--json`

The JSON is the internal report serialized as-is: field names match the Rust
structs, which in turn keep Roblox's own names where they exist. It is stable
in the sense that fields are added rather than renamed, but this is a 0.x tool
and that is a statement of intent, not a contract yet.

```sh
rbx-observe storefront 1111111111111 --json | jq '.summary'
```
