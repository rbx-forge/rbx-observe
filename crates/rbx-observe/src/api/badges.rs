//! `badges.roblox.com` — the badges an experience publishes.
//!
//! Cursor pagination: `cursor` in, `nextPageCursor` out, loop until it is
//! null. The developer-products endpoint uses the same idiom under a different
//! array key; game passes use tokens instead. See `docs/endpoints.md`.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::Client;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BadgeStatistics {
    #[serde(default)]
    pub past_day_awarded_count: u64,
    #[serde(default)]
    pub awarded_count: u64,
    /// Share of players who earned it, as a fraction. The API name says
    /// "percentage" and the value is 0..1, so the name is wrong at the source.
    #[serde(default)]
    pub win_rate_percentage: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Badge {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    /// Permanent asset id of the badge image.
    #[serde(default)]
    pub icon_image_id: Option<u64>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub statistics: BadgeStatistics,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BadgePage {
    #[serde(default = "Vec::new")]
    data: Vec<Badge>,
    #[serde(default)]
    next_page_cursor: Option<String>,
}

/// Roblox caps this at 100 and answers 400 above it.
const PAGE_SIZE: &str = "100";

/// A universe with more badges than this is either a badge farm or a bug in
/// the loop. Bailing beats walking a cursor that never ends.
const MAX_PAGES: usize = 200;

impl Client {
    pub async fn badges(&self, universe_id: u64) -> Result<Vec<Badge>> {
        let url = format!("{}/v1/universes/{universe_id}/badges", self.hosts().badges);
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;

        for _ in 0..MAX_PAGES {
            let mut params: Vec<(&str, &str)> = vec![("limit", PAGE_SIZE), ("sortOrder", "Asc")];
            if let Some(value) = cursor.as_deref() {
                params.push(("cursor", value));
            }

            let page: BadgePage = self.get_json(&url, &params).await?;
            out.extend(page.data);

            // An empty string is Roblox's "no more pages" as often as null is.
            match page.next_page_cursor {
                Some(next) if !next.is_empty() => cursor = Some(next),
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
    async fn pages_are_followed_and_the_cursor_is_sent_back() {
        let server = MockServer::start().await;

        // Page one carries no cursor. Asserting its absence is what proves the
        // loop starts clean rather than sending an empty cursor.
        Mock::given(method("GET"))
            .and(path("/v1/universes/42/badges"))
            .and(query_param("limit", "100"))
            .and(query_param("sortOrder", "Asc"))
            .and(query_param_is_missing("cursor"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":1,"name":"First","enabled":true,"iconImageId":9,
                   "statistics":{"pastDayAwardedCount":5,"awardedCount":50,"winRatePercentage":0.5}}],
                   "nextPageCursor":"page+two=="}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        // The cursor is opaque base64 with `+` and `=` in it. wiremock matches
        // the decoded value, so this asserts the encoding round-trips — a
        // cursor formatted into the URL by hand would arrive mangled and the
        // API would hand back page one forever.
        Mock::given(method("GET"))
            .and(path("/v1/universes/42/badges"))
            .and(query_param("cursor", "page+two=="))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":2,"name":"Second","enabled":false}],"nextPageCursor":null}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let badges = Client::with_base_url(&server.uri())
            .badges(42)
            .await
            .unwrap();

        assert_eq!(badges.len(), 2);
        assert_eq!(badges[0].icon_image_id, Some(9));
        assert_eq!(badges[0].statistics.awarded_count, 50);
        assert!(!badges[1].enabled);
    }

    #[tokio::test]
    async fn an_empty_cursor_ends_the_walk() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/universes/42/badges"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"data":[{"id":1,"name":"Only"}],"nextPageCursor":""}"#),
            )
            .expect(1)
            .mount(&server)
            .await;

        let badges = Client::with_base_url(&server.uri())
            .badges(42)
            .await
            .unwrap();
        assert_eq!(badges.len(), 1);
    }
}
