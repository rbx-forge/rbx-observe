//! `games.roblox.com/v1/games/{placeId}/servers/Public` — the servers running
//! right now.
//!
//! # What is not read
//!
//! Each server entry also carries `playerTokens` and `players`. Anonymously
//! both come back empty, so there is nothing to drop today — but they are the
//! fields that would turn a server list into a list of who is playing, so the
//! structs below do not declare them. If Roblox ever starts filling them for
//! anonymous callers, this file keeps ignoring them by construction rather
//! than by luck.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::Client;

#[derive(Deserialize)]
struct ServerEntry {
    #[serde(default)]
    playing: u64,
    #[serde(rename = "maxPlayers", default)]
    max_players: u64,
    #[serde(default)]
    fps: Option<f64>,
    #[serde(default)]
    ping: Option<f64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerPage {
    #[serde(default = "Vec::new")]
    data: Vec<ServerEntry>,
    #[serde(default)]
    next_page_cursor: Option<String>,
}

/// Live occupancy of one place, aggregated. Individual servers are summed
/// rather than listed: a server id is operational trivia, and the shape of the
/// fleet is the actual signal.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ServerSummary {
    pub servers: usize,
    pub playing: u64,
    pub capacity: u64,
    /// Mean over the servers that reported one. `None` when none did.
    pub average_fps: Option<f64>,
    pub average_ping: Option<f64>,
}

impl ServerSummary {
    /// Occupied seats over total seats, as a fraction. `None` on an empty
    /// fleet, where the ratio would be a division by zero dressed up as a
    /// number.
    pub fn fill_rate(&self) -> Option<f64> {
        (self.capacity > 0).then(|| self.playing as f64 / self.capacity as f64)
    }
}

/// Enough pages to summarise a busy place without walking a fleet of
/// thousands: each page is 100 servers.
const MAX_PAGES: usize = 5;
const PAGE_SIZE: &str = "100";

impl Client {
    /// Takes a **place** id, not a universe id: servers belong to a place, and
    /// a universe with several places runs several independent fleets.
    pub async fn place_servers(&self, place_id: u64) -> Result<ServerSummary> {
        let url = format!("{}/v1/games/{place_id}/servers/Public", self.hosts().games);
        let mut entries: Vec<ServerEntry> = Vec::new();
        let mut cursor: Option<String> = None;

        for _ in 0..MAX_PAGES {
            let mut params: Vec<(&str, &str)> = vec![("limit", PAGE_SIZE)];
            if let Some(value) = cursor.as_deref() {
                params.push(("cursor", value));
            }

            let page: ServerPage = self.get_json(&url, &params).await?;
            entries.extend(page.data);

            match page.next_page_cursor {
                Some(next) if !next.is_empty() => cursor = Some(next),
                _ => break,
            }
        }

        let mean = |values: Vec<f64>| {
            (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
        };

        Ok(ServerSummary {
            servers: entries.len(),
            playing: entries.iter().map(|server| server.playing).sum(),
            capacity: entries.iter().map(|server| server.max_players).sum(),
            average_fps: mean(entries.iter().filter_map(|server| server.fps).collect()),
            average_ping: mean(entries.iter().filter_map(|server| server.ping).collect()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn servers_are_summed_and_player_fields_are_never_parsed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/games/777/servers/Public"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[
                   {"id":"a","maxPlayers":8,"playing":8,"fps":59.8,"ping":120.0,
                    "playerTokens":["tok1","tok2"],"players":[{"id":1}]},
                   {"id":"b","maxPlayers":8,"playing":2,"fps":60.2,"ping":80.0,
                    "playerTokens":[],"players":[]}],
                   "nextPageCursor":null}"#,
            ))
            .mount(&server)
            .await;

        let summary = Client::with_base_url(&server.uri())
            .place_servers(777)
            .await
            .unwrap();

        assert_eq!(summary.servers, 2);
        assert_eq!(summary.playing, 10);
        assert_eq!(summary.capacity, 16);
        assert_eq!(summary.fill_rate(), Some(0.625));

        // The payload carries player tokens; the summary cannot expose what it
        // never declared.
        let json = serde_json::to_string(&summary).unwrap();
        assert!(!json.contains("tok1"), "{json}");
        assert!(!json.contains("player"), "{json}");
    }

    #[tokio::test]
    async fn a_place_with_no_servers_summarises_to_zero_not_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/games/778/servers/Public"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"data":[],"nextPageCursor":null}"#),
            )
            .mount(&server)
            .await;

        let summary = Client::with_base_url(&server.uri())
            .place_servers(778)
            .await
            .unwrap();

        assert_eq!(summary.servers, 0);
        assert_eq!(summary.fill_rate(), None);
        assert_eq!(summary.average_fps, None);
    }
}
