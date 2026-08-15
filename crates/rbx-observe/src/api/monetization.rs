//! `apis.roblox.com` — game passes and developer products.
//!
//! The two endpoints sit on the same host, describe the same kind of thing,
//! and disagree on everything mechanical:
//!
//! | | developer products | game passes |
//! |---|---|---|
//! | page size param | `limit` | `pageSize` |
//! | continuation in | `cursor` | `pageToken` |
//! | continuation out | `nextPageCursor` | `nextPageToken` |
//! | array key | `developerProducts` | `gamePasses` |
//! | field casing | `PascalCase` | `camelCase` |
//!
//! None of that is a mistake in this file. It is what the API does.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::Client;

/// Both endpoints cap at 100.
const PAGE_SIZE: &str = "100";
const MAX_PAGES: usize = 200;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeveloperProduct {
    #[serde(rename = "ProductId")]
    pub product_id: u64,
    #[serde(rename = "DeveloperProductId")]
    pub developer_product_id: u64,
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Description", default)]
    pub description: Option<String>,
    /// Robux. `None` on a product that is not for sale.
    #[serde(rename = "PriceInRobux", default)]
    pub price_in_robux: Option<u64>,
    #[serde(rename = "IsForSale", default)]
    pub is_for_sale: bool,
    /// Permanent asset id of the product icon.
    #[serde(rename = "IconImageAssetId", default)]
    pub icon_image_asset_id: Option<u64>,
    #[serde(rename = "Created", default)]
    pub created: Option<String>,
    #[serde(rename = "Updated", default)]
    pub updated: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePass {
    pub id: u64,
    #[serde(default)]
    pub product_id: Option<u64>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_description: Option<String>,
    /// Robux. `None` when the pass is off sale, which is distinct from free.
    #[serde(default)]
    pub price: Option<u64>,
    #[serde(default)]
    pub is_for_sale: bool,
    /// Permanent asset id of the pass icon.
    #[serde(default)]
    pub display_icon_image_asset_id: Option<u64>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProductPage {
    #[serde(rename = "developerProducts", default = "Vec::new")]
    products: Vec<DeveloperProduct>,
    #[serde(default)]
    next_page_cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PassPage {
    #[serde(default = "Vec::new")]
    game_passes: Vec<GamePass>,
    #[serde(default)]
    next_page_token: Option<String>,
}

impl Client {
    pub async fn developer_products(&self, universe_id: u64) -> Result<Vec<DeveloperProduct>> {
        let url = format!(
            "{}/developer-products/v2/universes/{universe_id}/developerproducts",
            self.hosts().apis
        );
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;

        for _ in 0..MAX_PAGES {
            let mut params: Vec<(&str, &str)> = vec![("limit", PAGE_SIZE)];
            if let Some(value) = cursor.as_deref() {
                params.push(("cursor", value));
            }

            let page: ProductPage = self.get_json(&url, &params).await?;
            out.extend(page.products);

            match page.next_page_cursor {
                Some(next) if !next.is_empty() => cursor = Some(next),
                _ => break,
            }
        }

        Ok(out)
    }

    /// `passView=Full` is not optional: without it the response is a summary
    /// with no price and no icon, which is most of the reason to ask.
    pub async fn game_passes(&self, universe_id: u64) -> Result<Vec<GamePass>> {
        let url = format!(
            "{}/game-passes/v1/universes/{universe_id}/game-passes",
            self.hosts().apis
        );
        let mut out = Vec::new();
        let mut token: Option<String> = None;

        for _ in 0..MAX_PAGES {
            let mut params: Vec<(&str, &str)> = vec![("passView", "Full"), ("pageSize", PAGE_SIZE)];
            if let Some(value) = token.as_deref() {
                params.push(("pageToken", value));
            }

            let page: PassPage = self.get_json(&url, &params).await?;
            out.extend(page.game_passes);

            match page.next_page_token {
                Some(next) if !next.is_empty() => token = Some(next),
                _ => break,
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn products_walk_cursors_and_keep_pascal_case_fields() {
        let server = MockServer::start().await;
        let endpoint = "/developer-products/v2/universes/5/developerproducts";

        Mock::given(method("GET"))
            .and(path(endpoint))
            .and(query_param("limit", "100"))
            .and(query_param_is_missing("cursor"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"developerProducts":[{"ProductId":5555555555553,"DeveloperProductId":5555555555556,
                   "Name":"2X Money","PriceInRobux":49,"IsForSale":true,
                   "IconImageAssetId":4444444444444448,"Created":"2026-06-29T06:45:43.037Z"}],
                   "nextPageCursor":"next"}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path(endpoint))
            .and(query_param("cursor", "next"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"developerProducts":[],"nextPageCursor":null}"#),
            )
            .expect(1)
            .mount(&server)
            .await;

        let products = Client::with_base_url(&server.uri())
            .developer_products(5)
            .await
            .unwrap();

        assert_eq!(products.len(), 1);
        assert_eq!(products[0].price_in_robux, Some(49));
        assert_eq!(products[0].icon_image_asset_id, Some(4444444444444448));
    }

    #[tokio::test]
    async fn passes_send_full_view_and_walk_tokens_not_cursors() {
        let server = MockServer::start().await;
        let endpoint = "/game-passes/v1/universes/5/game-passes";

        Mock::given(method("GET"))
            .and(path(endpoint))
            // The three things this endpoint does differently, asserted so a
            // refactor cannot quietly turn them into the cursor idiom.
            .and(query_param("passView", "Full"))
            .and(query_param("pageSize", "100"))
            .and(query_param_is_missing("pageToken"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"gamePasses":[{"id":5555555555552,"productId":5555555555554,"name":"skin pack girl",
                   "isForSale":true,"price":100,"displayIconImageAssetId":4444444444444445}],
                   "nextPageToken":"tok"}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path(endpoint))
            .and(query_param("pageToken", "tok"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"gamePasses":[],"nextPageToken":""}"#),
            )
            .expect(1)
            .mount(&server)
            .await;

        let passes = Client::with_base_url(&server.uri())
            .game_passes(5)
            .await
            .unwrap();

        assert_eq!(passes.len(), 1);
        assert_eq!(passes[0].price, Some(100));
        assert_eq!(passes[0].display_icon_image_asset_id, Some(4444444444444445));
    }
}
