//! `apis.roblox.com/explore-api` — the discovery page's own rankings.
//!
//! The one endpoint here that hands out universe ids rather than needing one,
//! which makes it the entry point to every other command: you cannot observe a
//! category you cannot enumerate.
//!
//! Page one carries the six obvious sorts. The rest of the pages carry the ones
//! that differentiate the tool — `top-earning` is a revenue proxy Roblox
//! publishes for free, and the fourteen `trending-in-<category>` sorts are its
//! real genre taxonomy, which the `genre` field on a game does not give you.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::Client;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SortGame {
    pub universe_id: u64,
    #[serde(default)]
    pub root_place_id: Option<u64>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub player_count: u64,
    #[serde(default)]
    pub total_up_votes: u64,
    #[serde(default)]
    pub total_down_votes: u64,
    /// Roblox marks paid placements in its own rankings. Worth carrying: a
    /// sponsored row is an ad, not a measurement.
    #[serde(default)]
    pub is_sponsored: bool,
    #[serde(default)]
    pub minimum_age: Option<u32>,
    #[serde(default)]
    pub content_maturity: Option<String>,
    #[serde(default)]
    pub genre_l1: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sort {
    #[serde(default)]
    pub sort_id: String,
    /// `Games`, or something else entirely: page one opens with a `Filters`
    /// entry that is UI furniture rather than a ranking.
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub sort_display_name: Option<String>,
    #[serde(default = "Vec::new")]
    pub games: Vec<SortGame>,
}

impl Sort {
    /// `trending-in-obby-and-platformer` → `obby-and-platformer`.
    pub fn category(&self) -> Option<&str> {
        self.sort_id.strip_prefix("trending-in-")
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SortsPage {
    #[serde(default = "Vec::new")]
    sorts: Vec<Sort>,
    #[serde(default)]
    next_sorts_page_token: Option<String>,
}

/// Five pages covers the 26 game sorts Roblox currently ships; the cap is a
/// stop, not a target.
const MAX_PAGES: usize = 8;

impl Client {
    /// Every ranking Roblox is currently showing, in page order.
    ///
    /// The `sessionId` is required and is not authentication: it is how Roblox
    /// keeps one browsing session's pagination consistent. A fresh one per run
    /// is correct, and deliberately carries nothing identifying.
    pub async fn sorts(&self) -> Result<Vec<Sort>> {
        let url = format!("{}/explore-api/v1/get-sorts", self.hosts().apis);
        let session = session_id();
        let mut out = Vec::new();
        let mut token: Option<String> = None;

        for _ in 0..MAX_PAGES {
            let mut params: Vec<(&str, &str)> = vec![("sessionId", &session)];
            if let Some(value) = token.as_deref() {
                params.push(("sortsPageToken", value));
            }

            let page: SortsPage = self.get_json(&url, &params).await?;
            out.extend(page.sorts.into_iter().filter(|sort| {
                // Page one opens with a `Filters` entry that has no games.
                sort.content_type == "Games" && !sort.games.is_empty()
            }));

            match page.next_sorts_page_token {
                Some(next) if !next.is_empty() => token = Some(next),
                _ => break,
            }
        }

        Ok(out)
    }
}

fn session_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default();
    format!("rbx-observe-{millis}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn pages_are_walked_and_non_game_sorts_dropped() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/explore-api/v1/get-sorts"))
            .and(query_param_is_missing("sortsPageToken"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"sorts":[
                   {"sortId":"filters_v5","contentType":"Filters","games":[]},
                   {"sortId":"top-playing-now","contentType":"Games","sortDisplayName":"Top Playing Now",
                    "games":[{"universeId":1111111111114,"rootPlaceId":2222222222222225,"name":"Manor Mystery",
                              "playerCount":848990,"totalUpVotes":10315681,"totalDownVotes":1037960,
                              "isSponsored":false,"minimumAge":0,"contentMaturity":"moderate",
                              "genreL1":"Survival"}]}],
                   "nextSortsPageToken":"page2"}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/explore-api/v1/get-sorts"))
            .and(query_param("sortsPageToken", "page2"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"sorts":[{"sortId":"trending-in-obby-and-platformer","contentType":"Games",
                    "sortDisplayName":"Trending in Obby","games":[
                      {"universeId":7,"name":"Tower","playerCount":10}]}],
                   "nextSortsPageToken":null}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let sorts = Client::with_base_url(&server.uri()).sorts().await.unwrap();

        // The `Filters` entry is UI furniture and must not be reported as a
        // ranking with zero games.
        assert_eq!(sorts.len(), 2);
        assert_eq!(sorts[0].sort_id, "top-playing-now");
        assert_eq!(sorts[0].games[0].player_count, 848990);
        assert_eq!(sorts[1].category(), Some("obby-and-platformer"));
    }

    #[test]
    fn the_session_id_carries_nothing_identifying() {
        let id = session_id();
        assert!(id.starts_with("rbx-observe-"), "{id}");
        assert!(id.len() < 40, "{id}");
    }
}
