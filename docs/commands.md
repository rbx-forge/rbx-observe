# Commands

Eight commands. Five of them share one shape:

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
rbx-observe game <universe-id>
rbx-observe game https://www.roblox.com/games/2222222222222221/Sandbox Frontier
```

- **Description** — the page's own text, trimmed to 600 characters. `--json`
  carries it in full.
- **Audience** — players right now, lifetime visits, favorites, up/down votes
  with an approval percentage.
- **Shape** — max players, Roblox's own genre taxonomy (`genre_l1 / genre_l2`,
  not the legacy `genre` field that says "All" almost everywhere), the
  **maturity label** (`Minimal`, `Mild (9+)`, …) with the content descriptors
  behind it, creation and last-update dates, paid-access price if there is one.
- **Places** — every place in the universe, not just the root one the page
  shows: a tutorial, a lobby, a staging copy. Roblox does **not** expose
  per-place public/private anonymously, and the output says so rather than
  guessing.
- **Assets** — the icon's asset id, the banner's asset id, the carousel size,
  and whether the page opens on a video.

Eight endpoints across five hosts. Three of them are enrichments — the icon
asset id, the maturity label, the place list — and a failure on any of those
degrades its section instead of failing the command.

## `storefront`

What the experience sells.

```sh
rbx-observe storefront <universe-id> --json
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
rbx-observe badges <universe-id>
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
rbx-observe media <universe-id> --json
```

Each screenshot's permanent asset id with its render URL, alt text when the
developer set one, and the preview video if there is one. Video entries are
excluded from the screenshot list: they carry a poster-frame `imageId` that
would otherwise inflate every carousel with a video by one.

## `charts`

What Roblox is pushing right now. **The discovery command** — every other one
needs a universe id, this one hands them out.

```sh
rbx-observe charts                              # every sort, 10 rows each
rbx-observe charts --sort top-earning --limit 25
rbx-observe charts --category obby-and-platformer
```

```
Top Earning
  top-earning · 94 games
    1.   376 346  Sea Trials                      universe 1111111111115 · RPG
    2.   105 347  Warrior Saga   universe 1111111111116 · Strategy
```

Roughly 26 rankings over 5 pages. The obvious ones are on page one
(`top-trending`, `up-and-coming`, `top-playing-now`); the ones worth the walk
are further in — **`top-earning` is a revenue proxy Roblox publishes for
free**, and the fourteen `trending-in-<category>` sorts are its real genre
taxonomy, which the `genre` field on a game does not give you.

`--category` matches the suffix of those sort ids. Paid placements are marked
`[sponsored]`: a sponsored row is an ad, not a measurement.

`--limit` truncates the **rendering** only; `--json` always carries every game
Roblox returned.

A ranking read once is a snapshot. Tracking one over time is a different tool
with a database, and this command deliberately is not that.

## `places`

Every place in the universe, with what an anonymous caller can work out about
each one.

```sh
rbx-observe places <universe-id>
```

```
Places
  2222222222222222  Harbour Patrol
    [root, published]
    12 server(s) · 79 playing / 96 seats · 82% full
  2222222222222223  Tutorial
    [not published]
    no live servers
```

`published` is an **inference**, not a Roblox flag: a place carries a product
id once it has been published to the site, and an internal one carries zero.
The authoritative `isPlayable` needs a session. The live fleet is the
corroborating signal — an internal place generally has no servers.

Separate from `game` because it costs two extra requests per place, and neither
of them batches.

## `group`

A studio and its catalog.

```sh
rbx-observe group <group-id>        # from roblox.com/communities/<groupId>/...
rbx-observe group <group-id> --all
```

Name, member count, whether it is open to join, description, then its games —
lifetime visits, **players right now**, and the `rbx-observe game <id>` line
that drills into each one. The whole catalog's live players cost one request,
not one per game: the details endpoint takes 50 universe ids at a time.

**Unlisted games are counted and, by default, not named.** Roblox hands an
anonymous caller a group's staging copies, test places and unreleased projects
— one measured group returns 2 games filtered to public and 21 unfiltered. The
summary always says how many there are; `--all` prints them, marked
`[not listed]`.

The group's **owner and shout are not read** even though Roblox sends them in
the same payload, and neither is the membership roster. There is also no
user-keyed twin of this command: `games.roblox.com/v2/users/{userId}/games`
exists and is public, and a catalog keyed to an individual account is a
person's output rather than a studio's.

## `asset`

Asset ids in, image URLs out — the companion to every other command, since they
all print asset ids.

```sh
rbx-observe asset <asset-id>
rbx-observe asset <asset-id> <asset-id> --size 1024x1024 --json
```

One batched call for as many ids as you pass. `--size` accepts `50x50`,
`128x128`, `150x150`, `256x256`, `420x420` (default), `512x512`, `1024x1024`,
and is validated before any request goes out.

**A URL is only reported for a completed render.** A nonexistent id, or one
naming something with no image, comes back from Roblox as `state: "Error"`
*with* an `imageUrl` — the grey placeholder, identical for every bad id. Those
rows print their state instead:

```
Assets at 512x512
  4444444444444441
    https://tr.rbxcdn.com/180DAY-c0cd3ed2c46b720a285fc6333bb59d8d/512/512/Image/Png/noFilter
  999999999999999  Error
```

An id Roblox omits from the response entirely still gets a row, marked
`Unanswered`, so the output always lines up with what you asked for.

This resolves the rendered image, not the original uploaded file: that one is
behind `assetdelivery.roblox.com`, which requires a session.

## Reading the output

One line per item, everywhere: a right-aligned number (price, awards, players),
the name, then ids in grey. **No URLs.** An asset id is printed as an id and
`rbx-observe asset` turns it into an image — the CDN URL cannot be built from
an id anyway (opaque hash, and the path segment varies by asset type), so a URL
printed on every row would have been the API call rather than the picture.

Each listing ends with the command that renders its icons:

```
  render icons  rbx-observe asset 4444444444444445 4444444444444446 4444444444444447 …
```

## Exit codes

`0` on success, `1` on any error. There is no drift concept here — nothing is
being reconciled, so there is no third state to report.

## `--json`

The JSON is the internal report serialized as-is: field names match the Rust
structs, which in turn keep Roblox's own names where they exist. It is stable
in the sense that fields are added rather than renamed, but this is a 0.x tool
and that is a statement of intent, not a contract yet.

```sh
rbx-observe storefront <universe-id> --json | jq '.summary'
```
