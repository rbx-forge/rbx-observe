//! `rbx-observe places` — every place in a universe, with what can be inferred
//! about each one.
//!
//! Separate from `game` because it costs two extra requests **per place**: the
//! place asset for the published inference, and the server list for live
//! occupancy. On a universe with twenty places that is forty paced requests,
//! which is not something to do behind someone's back on a command they ran to
//! read a description.

use anyhow::Result;
use serde::Serialize;

use crate::api::servers::ServerSummary;
use crate::api::Client;
use crate::render::{dim, heading, thousands};

#[derive(Debug, Serialize)]
pub struct Places {
    pub universe_id: u64,
    pub root_place_id: u64,
    pub places: Vec<PlaceStatus>,
}

#[derive(Debug, Serialize)]
pub struct PlaceStatus {
    pub id: u64,
    pub name: String,
    pub is_root: bool,
    /// Inferred from the place asset carrying a product id. `None` when the
    /// place asset could not be read at all.
    ///
    /// This is an inference, not a Roblox flag — the authoritative
    /// `isPlayable` needs a session. See `api::economy::AssetDetails`.
    pub published: Option<bool>,
    pub servers: ServerSummary,
}

pub async fn collect(client: &Client, universe_id: u64) -> Result<Places> {
    let detail = client.game_detail(universe_id).await?;
    let places = client.universe_places(universe_id).await?;

    let mut out = Vec::with_capacity(places.len());
    for place in places {
        let published = client
            .place_asset_details(place.id)
            .await
            .ok()
            .map(|asset| asset.is_published());

        // A place nobody can join answers with an empty fleet rather than an
        // error, so a failure here is a network problem and not information.
        let servers = client.place_servers(place.id).await.unwrap_or_default();

        out.push(PlaceStatus {
            is_root: place.id == detail.root_place_id,
            id: place.id,
            name: place.name,
            published,
            servers,
        });
    }

    // Root first, then the busiest: the root is what the page links to, and
    // after that the interesting ones are the ones actually running.
    out.sort_by(|a, b| {
        b.is_root
            .cmp(&a.is_root)
            .then_with(|| b.servers.playing.cmp(&a.servers.playing))
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(Places {
        universe_id,
        root_place_id: detail.root_place_id,
        places: out,
    })
}

pub fn render(places: &Places) {
    println!("{}", heading("Places"));

    for place in &places.places {
        let tags = [
            place.is_root.then_some("root"),
            match place.published {
                Some(true) => Some("published"),
                Some(false) => Some("not published"),
                None => None,
            },
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");

        println!("  {}  {}", place.id, place.name);
        println!("    {}", dim(&format!("[{tags}]")));

        let servers = &place.servers;
        if servers.servers == 0 {
            println!("    {}", dim("no live servers"));
            continue;
        }

        let fill = servers
            .fill_rate()
            .map(|rate| format!(" · {:.0}% full", rate * 100.0))
            .unwrap_or_default();
        println!(
            "    {}",
            dim(&format!(
                "{} server(s) · {} playing / {} seats{}",
                thousands(servers.servers as u64),
                thousands(servers.playing),
                thousands(servers.capacity),
                fill
            ))
        );
    }

    println!();
    println!(
        "  {}",
        dim(
            "\"published\" is inferred from the place asset carrying a product id; \
             Roblox's own isPlayable flag needs a session"
        )
    );
}

pub async fn run(client: &Client, universe_id: u64, json: bool) -> Result<()> {
    let places = collect(client, universe_id).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&places)?);
    } else {
        render(&places);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn each_place_gets_its_published_inference_and_its_fleet() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/games"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"data":[{"id":42,"rootPlaceId":777,"name":"Game"}]}"#),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/universes/42/places"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":778,"name":"Tutorial"},{"id":777,"name":"Main"}],
                   "nextPageCursor":null}"#,
            ))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v2/assets/777/details"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"AssetTypeId":9,"AssetId":777,"ProductId":5555555555555}"#),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/assets/778/details"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"AssetTypeId":9,"AssetId":778,"ProductId":0}"#),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/games/777/servers/Public"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"maxPlayers":8,"playing":6}],"nextPageCursor":null}"#,
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/games/778/servers/Public"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"data":[],"nextPageCursor":null}"#),
            )
            .mount(&server)
            .await;

        let places = collect(&Client::with_base_url(&server.uri()), 42)
            .await
            .unwrap();

        // Root first even though the listing returned it second.
        assert!(places.places[0].is_root);
        assert_eq!(places.places[0].published, Some(true));
        assert_eq!(places.places[0].servers.playing, 6);

        assert_eq!(places.places[1].name, "Tutorial");
        assert_eq!(places.places[1].published, Some(false));
        assert_eq!(places.places[1].servers.servers, 0);
    }
}
