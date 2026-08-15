//! `economy.roblox.com` — asset details for a place.
//!
//! One field justifies this host: `IconImageAssetId`. It is the only public
//! way to get the **asset id** of an experience's icon. The
//! `thumbnails.roblox.com` endpoints return a CDN URL whose hash cannot be
//! reversed into an asset id, and `develop.roblox.com/v1/universes/{id}/icon`
//! answers 401 without authentication. Everything else in the response
//! duplicates `games.roblox.com/v1/games`.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use super::Client;

/// `Place` in Roblox's asset type table.
const ASSET_TYPE_PLACE: u64 = 9;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetDetails {
    #[serde(rename = "AssetId")]
    pub asset_id: u64,
    #[serde(rename = "AssetTypeId")]
    pub asset_type_id: u64,
    #[serde(rename = "Name", default)]
    pub name: Option<String>,
    #[serde(rename = "IconImageAssetId", default)]
    pub icon_image_asset_id: Option<u64>,
    /// Non-zero once a place has been published to the site as a product.
    /// See [`AssetDetails::is_published`].
    #[serde(rename = "ProductId", default)]
    pub product_id: u64,
    #[serde(rename = "Created", default)]
    pub created: Option<String>,
    #[serde(rename = "Updated", default)]
    pub updated: Option<String>,
}

impl AssetDetails {
    /// Whether the place looks published rather than internal.
    ///
    /// **A heuristic, not a flag Roblox exposes.** Measured on a universe with
    /// a public root and a private `Tutorial` place: the root answers
    /// `ProductId: 5555555555555` with `ProductType: "User Product"`, the
    /// tutorial answers `ProductId: 0` with `ProductType: null`. A place gets
    /// a product record when it is published to the site, so a zero means it
    /// never was.
    ///
    /// The authoritative field, `isPlayable` on
    /// `games.roblox.com/v1/games/multiget-place-details`, needs a session.
    /// This is the closest an anonymous caller gets, and callers should
    /// present it as an inference.
    pub fn is_published(&self) -> bool {
        self.product_id != 0
    }
}

impl Client {
    /// Reads the place asset behind an experience.
    ///
    /// **Takes a place id, never a universe id.** The two are separate
    /// numbering spaces and this endpoint does not police the difference: a
    /// universe id returns `200` with a completely unrelated asset — someone
    /// else's t-shirt, with a plausible name and a real creator. The
    /// `AssetTypeId` guard below is the only thing standing between that and a
    /// confidently wrong answer, which is why it is an error rather than a
    /// warning.
    pub async fn place_asset_details(&self, place_id: u64) -> Result<AssetDetails> {
        let url = format!("{}/v2/assets/{place_id}/details", self.hosts().economy);
        let details: AssetDetails = self.get_json(&url, &[]).await?;

        if details.asset_type_id != ASSET_TYPE_PLACE {
            bail!(
                "Asset {place_id} is type {}, not a Place ({ASSET_TYPE_PLACE}). This endpoint \
                 answers 200 for any asset id, so a universe id passed here returns real data \
                 about the wrong thing.",
                details.asset_type_id
            );
        }

        Ok(details)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn a_place_yields_its_icon_asset_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/assets/2222222222222221/details"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"AssetTypeId":9,"AssetId":2222222222222221,"Name":"Sandbox Frontier",
                   "IconImageAssetId":4444444444444441,"Created":"2026-07-03T17:35:39.839Z"}"#,
            ))
            .mount(&server)
            .await;

        let details = Client::with_base_url(&server.uri())
            .place_asset_details(2222222222222221)
            .await
            .unwrap();

        assert_eq!(details.icon_image_asset_id, Some(4444444444444441));
    }

    #[tokio::test]
    async fn a_product_id_separates_a_published_place_from_an_internal_one() {
        let server = MockServer::start().await;
        // Both recorded live from one universe: the root place the game page
        // links to, and a `Tutorial` place that is not published.
        Mock::given(method("GET"))
            .and(path("/v2/assets/2222222222222222/details"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"AssetTypeId":9,"AssetId":2222222222222222,"Name":"Harbour Patrol",
                   "ProductId":5555555555555,"ProductType":"User Product"}"#,
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/assets/2222222222222223/details"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"AssetTypeId":9,"AssetId":2222222222222223,"Name":"Tutorial",
                   "ProductId":0,"ProductType":null}"#,
            ))
            .mount(&server)
            .await;

        let client = Client::with_base_url(&server.uri());
        assert!(client
            .place_asset_details(2222222222222222)
            .await
            .unwrap()
            .is_published());
        assert!(!client
            .place_asset_details(2222222222222223)
            .await
            .unwrap()
            .is_published());
    }

    #[tokio::test]
    async fn a_universe_id_passed_here_is_refused_despite_the_200() {
        let server = MockServer::start().await;
        // The real recorded response for a universe id: HTTP 200, someone
        // else's asset, no error anywhere.
        Mock::given(method("GET"))
            .and(path("/v2/assets/1111111111122/details"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"AssetId":1111111111122,"Name":"Hi","AssetTypeId":1}"#),
            )
            .mount(&server)
            .await;

        let error = Client::with_base_url(&server.uri())
            .place_asset_details(1111111111122)
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("not a Place"), "{error}");
    }
}
