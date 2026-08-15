//! `games.roblox.com` — the experience itself, plus the place → universe
//! lookup that lives on `apis.roblox.com`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::Client;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Creator {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
    /// `User` or `Group`. Named `kind` because `type` is a keyword.
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetail {
    pub id: u64,
    pub root_place_id: u64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub creator: Creator,
    /// Paid-access price in Robux. `None` on a free experience, which is
    /// almost all of them.
    #[serde(default)]
    pub price: Option<u64>,
    #[serde(default)]
    pub playing: u64,
    #[serde(default)]
    pub visits: u64,
    #[serde(default)]
    pub max_players: u32,
    #[serde(default)]
    pub favorited_count: u64,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    /// The legacy genre field. Says "All" on the large majority of
    /// experiences, so `genre_l1`/`genre_l2` below are the ones worth reading.
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default, rename = "genre_l1")]
    pub genre_l1: Option<String>,
    #[serde(default, rename = "genre_l2")]
    pub genre_l2: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Votes {
    #[serde(default)]
    pub up_votes: u64,
    #[serde(default)]
    pub down_votes: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaEntry {
    #[serde(default)]
    pub asset_type_id: u64,
    #[serde(default)]
    pub asset_type: Option<String>,
    /// A real, permanent asset id — unlike the CDN hashes the thumbnail
    /// endpoints hand back, this one can be fed to any asset API.
    #[serde(default)]
    pub image_id: Option<u64>,
    #[serde(default)]
    pub video_hash: Option<String>,
    #[serde(default)]
    pub video_title: Option<String>,
    /// Newer payloads carry this instead of `videoHash`. Both are read because
    /// the two shapes coexist in the wild.
    #[serde(default)]
    pub video_id: Option<String>,
    #[serde(default)]
    pub approved: bool,
    #[serde(default)]
    pub alt_text: Option<String>,
}

impl MediaEntry {
    /// Asset type 86 is `GamePreviewVideo`. The batched thumbnail endpoint
    /// cannot answer this question: it returns the video's poster frame as a
    /// plain image, making a game with a video indistinguishable from one
    /// without.
    pub fn is_video(&self) -> bool {
        self.asset_type_id == 86 || self.asset_type.as_deref() == Some("GamePreviewVideo")
    }
}

#[derive(Deserialize)]
struct DataEnvelope<T> {
    #[serde(default = "Vec::new")]
    data: Vec<T>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UniverseOfPlace {
    universe_id: u64,
}

/// Universe ids per request on the batched endpoints. **50, not 100**: 100
/// answers `{"code":9,"message":"Too many universe IDs were requested."}`
/// despite what the docs suggest.
pub const BATCH: usize = 50;

impl Client {
    /// Details for many universes in as few requests as the endpoint allows.
    ///
    /// Ids Roblox has nothing for are simply absent from the response, so the
    /// result is not positionally aligned with the input — callers match on
    /// `id`.
    pub async fn game_details(&self, universe_ids: &[u64]) -> Result<Vec<GameDetail>> {
        let url = format!("{}/v1/games", self.hosts().games);
        let mut out = Vec::with_capacity(universe_ids.len());

        for chunk in universe_ids.chunks(BATCH) {
            let joined = chunk
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let body: DataEnvelope<GameDetail> =
                self.get_json(&url, &[("universeIds", &joined)]).await?;
            out.extend(body.data);
        }

        Ok(out)
    }

    /// One universe, the degenerate case of the batch above. An unknown id is
    /// not an error here, it is an empty array, so the "not found" message is
    /// ours to write.
    pub async fn game_detail(&self, universe_id: u64) -> Result<GameDetail> {
        self.game_details(&[universe_id])
            .await?
            .into_iter()
            .next()
            .with_context(|| {
                format!(
                    "No experience with universe id {universe_id}. If that number came from a \
                     game URL it is a place id — pass --place."
                )
            })
    }

    pub async fn votes(&self, universe_id: u64) -> Result<Votes> {
        let url = format!("{}/v1/games/votes", self.hosts().games);
        // The entry also carries `id`, which we already know: `Votes` reads
        // the two fields it needs and serde drops the rest.
        let body: DataEnvelope<Votes> = self
            .get_json(&url, &[("universeIds", &universe_id.to_string())])
            .await?;

        Ok(body.data.into_iter().next().unwrap_or_default())
    }

    /// The game page carousel. One request per experience — this endpoint
    /// takes no batch, which is why anything doing bulk work should treat it
    /// as the expensive one.
    pub async fn media(&self, universe_id: u64) -> Result<Vec<MediaEntry>> {
        let url = format!("{}/v2/games/{universe_id}/media", self.hosts().games);
        let body: DataEnvelope<MediaEntry> = self.get_json(&url, &[]).await?;
        Ok(body.data)
    }

    /// Place id → universe id. The entry point when all you have is a game
    /// URL, since URLs carry place ids.
    pub async fn universe_of_place(&self, place_id: u64) -> Result<u64> {
        let url = format!(
            "{}/universes/v1/places/{place_id}/universe",
            self.hosts().apis
        );
        let body: UniverseOfPlace = self
            .get_json(&url, &[])
            .await
            .with_context(|| format!("Failed to resolve place {place_id} to a universe"))?;
        Ok(body.universe_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn game_detail_sends_the_id_and_reads_the_first_entry() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/games"))
            .and(query_param("universeIds", "1111111111111"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":1111111111111,"rootPlaceId":2222222222222221,"name":"Sandbox Frontier",
                   "creator":{"id":33333333334,"name":"Northwind Studio","type":"Group"},
                   "playing":1412,"visits":1540435,"favoritedCount":4678,"maxPlayers":16,
                   "created":"2026-07-03T17:35:39.839Z","updated":"2026-07-28T20:44:59Z",
                   "genre_l1":"Simulation","genre_l2":"Sandbox"}]}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let detail = Client::with_base_url(&server.uri())
            .game_detail(1111111111111)
            .await
            .unwrap();

        assert_eq!(detail.root_place_id, 2222222222222221);
        assert_eq!(detail.creator.kind.as_deref(), Some("Group"));
        assert_eq!(detail.genre_l1.as_deref(), Some("Simulation"));
        assert_eq!(detail.playing, 1412);
    }

    #[tokio::test]
    async fn an_empty_array_becomes_a_message_naming_the_place_flag() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/games"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
            .mount(&server)
            .await;

        let error = Client::with_base_url(&server.uri())
            .game_detail(123)
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("--place"), "{error}");
    }

    #[tokio::test]
    async fn many_universes_travel_in_one_request_per_fifty() {
        let server = MockServer::start().await;
        let ids: Vec<u64> = (1..=60).collect();

        // 60 ids, cap of 50: two requests, and the first one must carry
        // exactly fifty. A single request would come back as an error from
        // Roblox rather than a truncated list.
        let first: String = (1..=50)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",");
        Mock::given(method("GET"))
            .and(path("/v1/games"))
            .and(query_param("universeIds", first))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"data":[{"id":1,"rootPlaceId":11,"name":"One"}]}"#),
            )
            .expect(1)
            .mount(&server)
            .await;

        let second: String = (51..=60)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",");
        Mock::given(method("GET"))
            .and(path("/v1/games"))
            .and(query_param("universeIds", second))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"data":[{"id":51,"rootPlaceId":51,"name":"Fifty-one"}]}"#),
            )
            .expect(1)
            .mount(&server)
            .await;

        let details = Client::with_base_url(&server.uri())
            .game_details(&ids)
            .await
            .unwrap();

        assert_eq!(details.len(), 2);
    }

    #[tokio::test]
    async fn votes_default_to_zero_when_the_array_is_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/games/votes"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
            .mount(&server)
            .await;

        let votes = Client::with_base_url(&server.uri()).votes(1).await.unwrap();
        assert_eq!(votes.up_votes, 0);
    }

    #[tokio::test]
    async fn media_detects_a_preview_video_in_either_shape() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/games/7/media"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[
                   {"assetTypeId":1,"assetType":"Image","imageId":4444444444444443,"approved":true},
                   {"assetTypeId":86,"assetType":"GamePreviewVideo","videoHash":"abc","approved":true}]}"#,
            ))
            .mount(&server)
            .await;

        let media = Client::with_base_url(&server.uri()).media(7).await.unwrap();

        assert_eq!(media.len(), 2);
        assert!(!media[0].is_video());
        assert!(media[1].is_video());
        assert_eq!(media[0].image_id, Some(4444444444444443));
    }

    #[tokio::test]
    async fn a_place_resolves_to_its_universe() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/universes/v1/places/1818/universe"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"universeId":1111111111121}"#),
            )
            .expect(1)
            .mount(&server)
            .await;

        let universe = Client::with_base_url(&server.uri())
            .universe_of_place(1818)
            .await
            .unwrap();

        assert_eq!(universe, 1111111111121);
    }
}
