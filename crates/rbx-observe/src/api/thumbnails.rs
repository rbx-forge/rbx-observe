//! `thumbnails.roblox.com` — rendered images.
//!
//! Two endpoints that look interchangeable and are not:
//!
//! - **icons** answers with `targetId = universeId`, so it gives a URL and no
//!   asset id. The icon's asset id has to come from `economy.rs`.
//! - **multiget/thumbnails** (the page carousel banner) answers with
//!   `targetId = the real asset id`, so nothing needs resolving afterwards.
//!
//! Both URLs are CDN links whose hash is not reversible into an asset id,
//! which is the whole reason the distinction matters.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::Client;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Thumbnail {
    /// For the icons endpoint this is the universe id; for the carousel
    /// endpoint it is the asset id. Read the module comment before using it.
    #[serde(default)]
    pub target_id: u64,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub image_url: Option<String>,
}

impl Thumbnail {
    fn is_usable(&self) -> bool {
        // Roblox reports a not-yet-rendered image as `Pending` with an empty
        // URL, and both halves of that have been seen alone.
        self.state == "Completed" && self.image_url.as_deref().is_some_and(|url| !url.is_empty())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThumbnailSet {
    #[serde(default = "Vec::new")]
    thumbnails: Vec<Thumbnail>,
}

#[derive(Deserialize)]
struct DataEnvelope<T> {
    #[serde(default = "Vec::new")]
    data: Vec<T>,
}

/// A universe icon and the game page banner, as the pair every caller wants.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Images {
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    /// The banner's asset id, straight from `targetId`. The icon has no
    /// equivalent here by design of the API.
    pub banner_asset_id: Option<u64>,
}

/// Sizes the thumbnail service accepts. Anything else answers 400, so the CLI
/// validates against this list rather than letting Roblox reject it after a
/// round-trip.
pub const SIZES: [&str; 7] = [
    "50x50",
    "128x128",
    "150x150",
    "256x256",
    "420x420",
    "512x512",
    "1024x1024",
];

/// Ids per request. The endpoint accepts more, but batches of 50 are what the
/// neighbouring games endpoints tolerate, and one number is easier to reason
/// about than five.
const BATCH: usize = 50;

impl Client {
    /// Resolves bare asset ids to the CDN URLs that render them.
    ///
    /// This is the only route from an asset id to an image without a
    /// credential: `assetdelivery.roblox.com`, which serves the *original*
    /// uploaded file rather than a render, answers 401 anonymously.
    ///
    /// `targetId` is the asset id here, so the mapping back to the input is
    /// direct — unlike the games/icons endpoint, where it is the universe id.
    pub async fn asset_thumbnails(&self, ids: &[u64], size: &str) -> Result<Vec<Thumbnail>> {
        let url = format!("{}/v1/assets", self.hosts().thumbnails);
        let mut out = Vec::with_capacity(ids.len());

        for chunk in ids.chunks(BATCH) {
            let joined = chunk
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");

            let body: DataEnvelope<Thumbnail> = self
                .get_json(
                    &url,
                    &[
                        ("assetIds", &joined),
                        ("size", size),
                        ("format", "Png"),
                        ("isCircular", "false"),
                    ],
                )
                .await?;

            out.extend(body.data);
        }

        Ok(out)
    }

    pub async fn images(&self, universe_id: u64) -> Result<Images> {
        let icon = self.icon(universe_id).await?;
        let banner = self.banner(universe_id).await?;

        Ok(Images {
            icon_url: icon.and_then(|t| t.image_url),
            banner_asset_id: banner.as_ref().map(|t| t.target_id),
            banner_url: banner.and_then(|t| t.image_url),
        })
    }

    async fn icon(&self, universe_id: u64) -> Result<Option<Thumbnail>> {
        let url = format!("{}/v1/games/icons", self.hosts().thumbnails);
        let body: DataEnvelope<Thumbnail> = self
            .get_json(
                &url,
                &[
                    ("universeIds", &universe_id.to_string()),
                    ("size", "512x512"),
                    ("format", "Png"),
                    ("isCircular", "false"),
                ],
            )
            .await?;

        Ok(body.data.into_iter().find(Thumbnail::is_usable))
    }

    /// `countPerUniverse=1`: only the first carousel image is worth reading.
    /// It is the one Roblox shows first, and the rest of the carousel changes
    /// far more often than it means anything.
    async fn banner(&self, universe_id: u64) -> Result<Option<Thumbnail>> {
        let url = format!("{}/v1/games/multiget/thumbnails", self.hosts().thumbnails);
        let body: DataEnvelope<ThumbnailSet> = self
            .get_json(
                &url,
                &[
                    ("universeIds", &universe_id.to_string()),
                    ("countPerUniverse", "1"),
                    ("defaults", "true"),
                    ("size", "768x432"),
                    ("format", "Png"),
                ],
            )
            .await?;

        Ok(body
            .data
            .into_iter()
            .flat_map(|set| set.thumbnails)
            .find(Thumbnail::is_usable))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn the_banner_carries_an_asset_id_and_the_icon_does_not() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/games/icons"))
            .and(query_param("universeIds", "42"))
            .and(query_param("size", "512x512"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"targetId":42,"state":"Completed","imageUrl":"https://tr.rbxcdn.com/icon"}]}"#,
            ))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/games/multiget/thumbnails"))
            .and(query_param("countPerUniverse", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"universeId":42,"error":null,"thumbnails":[
                   {"targetId":4444444444444443,"state":"Completed","imageUrl":"https://tr.rbxcdn.com/banner"}]}]}"#,
            ))
            .mount(&server)
            .await;

        let images = Client::with_base_url(&server.uri())
            .images(42)
            .await
            .unwrap();

        assert_eq!(
            images.icon_url.as_deref(),
            Some("https://tr.rbxcdn.com/icon")
        );
        assert_eq!(images.banner_asset_id, Some(4444444444444443));
    }

    #[tokio::test]
    async fn asset_ids_resolve_to_cdn_urls_in_one_batched_call() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/assets"))
            .and(query_param("assetIds", "4444444444444441,4444444444444442"))
            .and(query_param("size", "420x420"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[
                   {"targetId":4444444444444441,"state":"Completed",
                    "imageUrl":"https://tr.rbxcdn.com/180DAY-c0/420/420/Image/Png/noFilter"},
                   {"targetId":4444444444444442,"state":"Blocked","imageUrl":null}]}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let thumbnails = Client::with_base_url(&server.uri())
            .asset_thumbnails(&[4444444444444441, 4444444444444442], "420x420")
            .await
            .unwrap();

        assert_eq!(thumbnails.len(), 2);
        // A blocked asset is reported with its state rather than dropped: the
        // caller asked about that id and deserves the answer.
        assert_eq!(thumbnails[1].state, "Blocked");
        assert!(thumbnails[1].image_url.is_none());
    }

    #[tokio::test]
    async fn a_pending_thumbnail_is_skipped_rather_than_reported_as_an_image() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/games/icons"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    r#"{"data":[{"targetId":42,"state":"Pending","imageUrl":""}]}"#,
                ),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/games/multiget/thumbnails"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
            .mount(&server)
            .await;

        let images = Client::with_base_url(&server.uri())
            .images(42)
            .await
            .unwrap();

        assert!(images.icon_url.is_none());
        assert!(images.banner_asset_id.is_none());
    }
}
