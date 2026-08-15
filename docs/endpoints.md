# Endpoints

Every request this tool makes, why it makes it, and the traps around it.
**All of them are public**: no API key, no cookie, no authentication header of
any kind. If something here ever needs a credential, it does not belong in this
tool.

| Data | Host | Takes | Module |
|---|---|---|---|
| Experience details | `games.roblox.com` | universe id | `api/games.rs` |
| Votes | `games.roblox.com` | universe id | `api/games.rs` |
| Carousel media | `games.roblox.com` | universe id | `api/games.rs` |
| Place → universe | `apis.roblox.com` | **place id** | `api/games.rs` |
| Badges | `badges.roblox.com` | universe id | `api/badges.rs` |
| Developer products | `apis.roblox.com` | universe id | `api/monetization.rs` |
| Game passes | `apis.roblox.com` | universe id | `api/monetization.rs` |
| Content maturity | `apis.roblox.com` | universe id (POST) | `api/maturity.rs` |
| Places in a universe | `develop.roblox.com` | universe id | `api/places.rs` |
| Live servers | `games.roblox.com` | **place id** | `api/servers.rs` |
| Place asset details | `economy.roblox.com` | **place id** | `api/economy.rs` |
| Icon and banner images | `thumbnails.roblox.com` | universe id | `api/thumbnails.rs` |
| Any asset's render URL | `thumbnails.roblox.com` | **asset id** | `api/thumbnails.rs` |
| Group | `groups.roblox.com` | group id | `api/groups.rs` |
| A group's games | `games.roblox.com` | group id | `api/groups.rs` |

Real `roblox.com` hosts only. Community proxies like `roproxy.com` mirror
these paths and are popular for getting around IP rate limits; routing traffic
through a third party to dodge a quota is not what a polite reader does, and
the quota is a signal to slow down rather than an obstacle.

## Three numbering spaces

A universe id, a place id and an asset id are different numbers for different
things, and none of them is recognisable by looking at it.

- A **game URL carries a place id**: `roblox.com/games/<placeId>/<name>`.
- Every read endpoint here takes a **universe id**, except the two marked
  above.
- `rbx-observe` converts once, in `target.rs`, so no other module has to
  assume.

## Experience details

```http
GET https://games.roblox.com/v1/games?universeIds={universeId}
```

No pagination. Accepts a comma-separated list (50 ids per call in practice —
100 answers `Too many universe IDs were requested`).

An unknown id is **not an error**: the response is `{"data":[]}` with status
200, so the "no such experience" message is ours to write.

This is also where `rootPlaceId` comes from, which the two place-keyed
endpoints need.

`genre` is the legacy field and says `All` on the large majority of
experiences. `genre_l1` / `genre_l2` are the ones worth reading.

## Votes

```http
GET https://games.roblox.com/v1/games/votes?universeIds={universeId}
```

Same batching. Missing entries mean zero, not an error.

## Badges

```http
GET https://badges.roblox.com/v1/universes/{universeId}/badges
    ?limit=100&sortOrder=Asc&cursor={cursor}
```

**Cursor pagination**: array under `data`, continuation in `nextPageCursor`,
loop until it is null *or empty* — both have been observed as the terminator.

`statistics` is the interesting half: `awardedCount`, `pastDayAwardedCount`,
and `winRatePercentage`, which despite the name is a fraction between 0 and 1.

## Developer products

```http
GET https://apis.roblox.com/developer-products/v2/universes/{universeId}/developerproducts
    ?limit=100&cursor={cursor}
```

Cursor pagination like badges, but the array key is `developerProducts` and the
fields are `PascalCase`.

`IconImageAssetId` is a real, permanent asset id.

## Game passes

```http
GET https://apis.roblox.com/game-passes/v1/universes/{universeId}/game-passes
    ?passView=Full&pageSize=100&pageToken={token}
```

The endpoint that disagrees with every other one:

- **tokens, not cursors** (`pageToken` in, `nextPageToken` out)
- `pageSize`, not `limit`
- array key `gamePasses`, fields `camelCase`

`passView=Full` is required. Without it the response is a summary with no price
and no icon, which is most of the reason to ask.

## Place asset details

```http
GET https://economy.roblox.com/v2/assets/{rootPlaceId}/details
```

Exactly one field justifies this host: **`IconImageAssetId`**, the only public
way to get the *asset id* of an experience's icon. The thumbnail endpoints
return a CDN URL whose hash is not reversible into an asset id, and
`develop.roblox.com/v1/universes/{id}/icon` answers 401 without authentication.
Everything else in the response duplicates the games endpoint.

> **The trap.** This endpoint takes an **asset id** and does not police what you
> send it. A universe id returns `200` with a completely unrelated asset —
> someone else's shirt, with a plausible name and a real creator. There is no
> error anywhere in the exchange.
>
> `rbx-observe` refuses any response whose `AssetTypeId` is not `9` (Place).
> That guard is the only thing between this endpoint and a confidently wrong
> answer.

## Images

```http
GET https://thumbnails.roblox.com/v1/games/icons
    ?universeIds={id}&size=512x512&format=Png&isCircular=false

GET https://thumbnails.roblox.com/v1/games/multiget/thumbnails
    ?universeIds={id}&countPerUniverse=1&defaults=true&size=768x432&format=Png
```

They look interchangeable and are not:

- **icons** answers with `targetId = universeId`. You get a URL and no asset id.
- **multiget/thumbnails** (the page banner) answers with `targetId = the asset
  id`. Nothing to resolve afterwards.

A not-yet-rendered image comes back as `state: "Pending"`, sometimes with an
empty URL and sometimes with no URL at all. Both are skipped rather than
reported as an image.

Valid sizes: `50x50`, `128x128`, `150x150`, `256x256`, `420x420`, `512x512`,
`1024x1024`.

### Rendering any asset id

Every asset id this tool prints can be turned back into an image:

```http
GET https://thumbnails.roblox.com/v1/assets?assetIds={assetId}&size=420x420&format=Png
```

The same asset rendered through the assets endpoint and through the games
endpoint shares one CDN hash, which is how you can verify an icon asset id is
really the icon.

## Carousel media

```http
GET https://games.roblox.com/v2/games/{universeId}/media
```

**Not batched** — one request per experience, which makes it the expensive one
for anything doing bulk work.

`imageId` is a permanent asset id. Asset type `86` (`GamePreviewVideo`) is a
preview video: the batched thumbnail endpoint cannot answer that question,
because it returns the video's poster frame as a plain image, making a game
with a video indistinguishable from one without.

A video entry also carries an `imageId` for its poster frame, so it is excluded
from the screenshot list rather than counted twice.

## Content maturity

```http
POST https://apis.roblox.com/experience-guidelines-api/experience-guidelines/get-age-recommendation
{"universeId": 1111111111111}
```

The only POST in this tool — the universe id travels in a body rather than a
query string — and still a read.

Answers the label the game page shows (`Minimal`, `Mild`, `Moderate`,
`Restricted`), a `minimumAge`, and `descriptorUsages`: the content descriptors
Roblox checked, each with a `contains` flag. **Descriptors with
`contains: false` are ones it checked and did not find**; printing them would
say a game contains things it does not, so they are filtered out.

An experience Roblox has not rated yet answers `{}`, which is not an error.

## Places in a universe

```http
GET https://develop.roblox.com/v1/universes/{universeId}/places?limit=100&cursor={cursor}
```

Cursor pagination. An experience is one universe holding a root place plus,
often, others: a tutorial, a lobby, a staging copy. The game page only ever
shows the root, so this is the only view of how many rooms the building has.

Most of `develop.roblox.com` needs a session; this listing does not, which is
unusual enough to be worth stating. The place-level endpoint beside it,
`/v1/places/{placeId}`, answers 404 anonymously.

**It does not say whether a place is public or private.** Roblox exposes no
per-place visibility flag anonymously — `isPlayable` lives on
`games.roblox.com/v1/games/multiget-place-details`, which answers
`Authentication token is missing`. Anonymity buys existence, name and
description, and nothing further.

## Group and its games

```http
GET https://groups.roblox.com/v1/groups/{groupId}
GET https://games.roblox.com/v2/groups/{groupId}/gamesV2?accessFilter=2&limit=100&cursor={cursor}
```

A studio and its catalog. `accessFilter=2` is "Public"; without it the listing
mixes in entries an anonymous caller cannot see. `gamesV2` rather than `games`
because the v1 shape omits `rootPlace`, which is what makes an entry openable.

The group payload also carries an `owner` object and a `shout` whose author is
a user. **Neither is declared in the structs**, so serde drops them and the
data never enters the process. Same for `/v1/groups/{id}/roles` and
`/v1/groups/{id}/roles/{roleId}/users`: the membership roster is public and is
not read.

## Resolving any asset id to an image

```http
GET https://thumbnails.roblox.com/v1/assets?assetIds={id},{id}&size=420x420&format=Png
```

Batched, and the way `rbx-observe asset` works. `targetId` is the asset id
here, so results map straight back to the input.

> **The trap.** A nonexistent id, or one naming something with no image,
> answers `state: "Error"` **with an `imageUrl`** — the grey "no thumbnail"
> placeholder, byte-identical for every bad id. Trusting the URL rather than
> the state hands back a working link for a wrong id. Only `Completed` is
> reported as a URL.
>
> Recorded live: id `999999999999999` and id `1` both return
> `t2.rbxcdn.com/180DAY-a53354b1f60a5dedc00d0600d1491075`. An id of `0`
> returns an empty array instead, so a requested id can also be missing from
> the response entirely.

The URL is **not constructible from the asset id**: the hash is opaque and the
path segment varies by asset type (`/420/420/Image/Png/noFilter` versus
`/420/420/GameIcon9/Png/noFilter`). The call is mandatory.

This resolves the *rendered* image. The original uploaded file lives behind
`assetdelivery.roblox.com`, which answers 401 anonymously — that is what
`rbx download` in the sibling project needs a credential for.

## Live servers

```http
GET https://games.roblox.com/v1/games/{placeId}/servers/Public?limit=100&cursor={cursor}
```

Takes a **place** id: servers belong to a place, so a universe with several
places runs several independent fleets. Each entry carries `playing`,
`maxPlayers`, `fps` and `ping`, which `rbx-observe places` sums into a fleet
size, an occupancy and a fill rate.

Each entry also carries `playerTokens` and `players`. Anonymously both come
back **empty**, so there is nothing to drop today — but they are the fields
that would turn a server list into a list of who is playing, so the structs do
not declare them. If Roblox ever starts filling them for anonymous callers,
this stays ignored by construction rather than by luck.

## Is a place public?

There is no anonymous flag for it. The authoritative one, `isPlayable` on
`games.roblox.com/v1/games/multiget-place-details`, answers
`Authentication token is missing`.

What does work is an **inference** from the place asset:

| Place | `ProductId` | `ProductType` |
|---|---|---|
| published root place | `5555555555555` | `"User Product"` |
| internal `Tutorial` place | `0` | `null` |

A place gets a product record when it is published to the site, so a zero means
it never was. `rbx-observe places` reports this as `published` /
`not published` and says in the output that it is inferred. Live servers are
the corroborating signal: an internal place generally has none.

## Batching

Which endpoints take several ids at once, because the difference is one request
versus fifty:

| Endpoint | Batches on | Cap |
|---|---|---|
| `games/v1/games` | `universeIds` | **50** — 100 answers `Too many universe IDs were requested` |
| `games/v1/games/votes` | `universeIds` | 50 |
| `thumbnails/v1/assets` | `assetIds` | 50 used here |
| `thumbnails/v1/games/icons` | `universeIds` | 50 |
| `thumbnails/v1/games/multiget/thumbnails` | `universeIds` | 50 |

Everything else is one call per subject: badges, developer products, game
passes, carousel media, place asset details, live servers. `rbx-observe group`
uses the batch to price a whole catalog's live players in a single request;
`rbx-observe places` cannot, which is why it is a separate command rather than
a section of `game`.

## Rate limiting

The quota is **per host and per IP**, and it is a sliding window rather than a
flat rate: a long run trips 429 even at steady spacing, because the window
fills up regardless of how evenly requests arrive.

Measured against `games.roblox.com` with real ids: a short burst survives at
1 req/s, 429 appears around 400ms, and a sustained run at a fixed 1.5s still
lost about 8% of its batches. Requesting ids that do not exist gets you
throttled considerably harder, which is worth knowing before blaming the quota.

So the pacer starts at 1s, widens by 1.5× on every 429 up to 6s, and never
narrows within a process. Retries are 429 and 5xx only, with 1s/2s/4s backoff.

## Not used, deliberately

All of the following is public, anonymous, and would work. None of it is here,
because each one is a record about a person rather than about a game or a
studio. The boundary in the README does not bend for convenience, and this list
exists so that the next person to consider adding one finds the reasoning
first.

| Endpoint | What it would give | Why not |
|---|---|---|
| `presence.roproxy.com/v1/presence/users` | where an account is right now, down to the place | live location tracking of a person |
| `inventory.roblox.com/v2/users/{id}/inventory` | what an account owns | a person's possessions |
| `users.roblox.com/v1/users/{id}` | profile, join date, description | a person's identity record |
| `groups.roblox.com/v2/users/{id}/groups/roles` | every group an account belongs to, and its rank | a person's affiliations |
| `groups.roblox.com/v1/groups/{id}/roles/{roleId}/users` | the membership roster | a list of people |
| `games.roblox.com/v2/users/{userId}/games` | games published by an account | a catalog keyed to an individual; the group-keyed twin is used instead |
| `owner` / `shout` on `groups/v1/groups/{id}` | who runs the studio | a person, inside a payload we do read |

The last two are the interesting ones, because they are one field away from
data the tool already handles. The enforcement is structural: the fields are
not declared on the structs, so serde drops them, and a test asserts the owner
does not survive parsing.
