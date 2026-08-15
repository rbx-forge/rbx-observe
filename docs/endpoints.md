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
| Place asset details | `economy.roblox.com` | **place id** | `api/economy.rs` |
| Icon and banner images | `thumbnails.roblox.com` | universe id | `api/thumbnails.rs` |

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

`groups.roblox.com` exposes a group's owner, its rank list, and the members of
each rank. All of it is public and none of it is in this tool: those are
person-level records, and the boundary in the README does not bend for
convenience.
