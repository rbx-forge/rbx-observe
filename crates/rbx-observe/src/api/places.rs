//! `develop.roblox.com` — the places inside a universe.
//!
//! An experience is one universe with one root place plus, often, a handful of
//! others: a tutorial, a lobby, a staging copy the developer forgot to delete.
//! The root place is the only one the game page shows, so this is the one
//! endpoint that says how many rooms the building has.
//!
//! Most of `develop.roblox.com` needs a session. This listing does not, which
//! is unusual enough to be worth stating: the place-level endpoints next to it
//! (`/v1/places/{id}`) answer 404 anonymously.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::Client;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    pub id: u64,
    #[serde(default)]
    pub universe_id: Option<u64>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlacePage {
    #[serde(default = "Vec::new")]
    data: Vec<Place>,
    #[serde(default)]
    next_page_cursor: Option<String>,
}

const PAGE_SIZE: &str = "100";
const MAX_PAGES: usize = 50;

impl Client {
    /// Every place in the universe, root first if Roblox says so — the order
    /// is the API's and is not documented as meaningful, so callers that care
    /// about the root should compare ids with `rootPlaceId` rather than take
    /// the first entry.
    ///
    /// **This does not say whether a place is public or private.** Roblox
    /// exposes no per-place visibility flag anonymously: `isPlayable` lives on
    /// `multiget-place-details`, which requires a session. What anonymity buys
    /// is existence, name and description, and nothing further.
    pub async fn universe_places(&self, universe_id: u64) -> Result<Vec<Place>> {
        let url = format!("{}/v1/universes/{universe_id}/places", self.hosts().develop);
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;

        for _ in 0..MAX_PAGES {
            let mut params: Vec<(&str, &str)> = vec![("limit", PAGE_SIZE)];
            if let Some(value) = cursor.as_deref() {
                params.push(("cursor", value));
            }

            let page: PlacePage = self.get_json(&url, &params).await?;
            out.extend(page.data);

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
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn every_place_in_the_universe_is_listed_across_pages() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/universes/1111111111112/places"))
            .and(query_param("limit", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"previousPageCursor":null,"nextPageCursor":"p2","data":[
                   {"id":2222222222222223,"universeId":1111111111112,"name":"Tutorial",
                    "description":"first steps"}]}"#,
            ))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/universes/1111111111112/places"))
            .and(query_param("cursor", "p2"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"nextPageCursor":null,"data":[
                   {"id":2222222222222222,"universeId":1111111111112,"name":"Harbour Patrol"}]}"#,
            ))
            .mount(&server)
            .await;

        let places = Client::with_base_url(&server.uri())
            .universe_places(1111111111112)
            .await
            .unwrap();

        assert_eq!(places.len(), 2);
        assert_eq!(places[0].name, "Tutorial");
        assert_eq!(places[1].id, 2222222222222222);
    }
}
