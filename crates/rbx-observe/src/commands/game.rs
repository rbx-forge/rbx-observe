//! `rbx-observe game` — what the experience's page says about itself.

use anyhow::Result;
use serde::Serialize;

use crate::api::games::{GameDetail, Votes};
use crate::api::thumbnails::Images;
use crate::api::Client;
use crate::render::{asset_url, date, dim, heading, thousands};

#[derive(Debug, Serialize)]
pub struct Game {
    pub detail: GameDetail,
    pub votes: Votes,
    pub images: Images,
    /// Asset id of the experience icon, from the place asset. `None` when the
    /// place asset could not be read — the rest of the report is still worth
    /// printing, so this is not an error.
    pub icon_asset_id: Option<u64>,
    pub media_count: usize,
    pub has_preview_video: bool,
}

pub async fn collect(client: &Client, universe_id: u64) -> Result<Game> {
    let detail = client.game_detail(universe_id).await?;
    let votes = client.votes(universe_id).await?;
    let images = client.images(universe_id).await?;
    let media = client.media(universe_id).await?;

    // The icon's asset id needs a second host and the root place id, which is
    // only known once `detail` has come back. A failure here is degraded
    // output, not a failed run: everything above it is already usable.
    let icon_asset_id = client
        .place_asset_details(detail.root_place_id)
        .await
        .ok()
        .and_then(|asset| asset.icon_image_asset_id);

    Ok(Game {
        icon_asset_id,
        media_count: media.len(),
        has_preview_video: media.iter().any(|entry| entry.is_video()),
        detail,
        votes,
        images,
    })
}

pub fn render(game: &Game) {
    let d = &game.detail;

    println!("{}", heading(&d.name));
    println!(
        "  {} {} · universe {} · place {}",
        d.creator.kind.as_deref().unwrap_or("creator"),
        d.creator.name.as_deref().unwrap_or("unknown"),
        d.id,
        d.root_place_id
    );
    println!("  https://www.roblox.com/games/{}", d.root_place_id);
    println!();

    println!("{}", heading("Audience"));
    println!("  playing now   {}", thousands(d.playing));
    println!("  visits        {}", thousands(d.visits));
    println!("  favorites     {}", thousands(d.favorited_count));
    println!(
        "  votes         {} up / {} down{}",
        thousands(game.votes.up_votes),
        thousands(game.votes.down_votes),
        approval(&game.votes)
    );
    println!();

    println!("{}", heading("Shape"));
    println!("  max players   {}", d.max_players);
    println!("  genre         {}", genre(d));
    println!("  created       {}", date(d.created.as_deref()));
    println!("  updated       {}", date(d.updated.as_deref()));
    if let Some(price) = d.price {
        println!("  paid access   R$ {}", thousands(price));
    }
    println!();

    println!("{}", heading("Assets"));
    match game.icon_asset_id {
        Some(id) => println!(
            "  icon          {id}\n                {}",
            dim(&asset_url(id))
        ),
        None => println!("  icon          {}", dim("asset id unavailable")),
    }
    match game.images.banner_asset_id {
        Some(id) => println!(
            "  banner        {id}\n                {}",
            dim(&asset_url(id))
        ),
        None => println!("  banner        {}", dim("none")),
    }
    println!(
        "  carousel      {} item(s), {}",
        game.media_count,
        if game.has_preview_video {
            "opens on a video"
        } else {
            "no preview video"
        }
    );
}

/// Roblox's own taxonomy first: the legacy `genre` field says "All" on the
/// large majority of experiences, so it is the fallback, not the answer.
fn genre(detail: &GameDetail) -> String {
    match (&detail.genre_l1, &detail.genre_l2) {
        (Some(l1), Some(l2)) => format!("{l1} / {l2}"),
        (Some(l1), None) => l1.clone(),
        _ => detail.genre.clone().unwrap_or_else(|| "-".to_string()),
    }
}

fn approval(votes: &Votes) -> String {
    let total = votes.up_votes + votes.down_votes;
    if total == 0 {
        return String::new();
    }
    format!(
        " ({:.0}% approval)",
        (votes.up_votes as f64 / total as f64) * 100.0
    )
}

pub async fn run(client: &Client, universe_id: u64, json: bool) -> Result<()> {
    let game = collect(client, universe_id).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&game)?);
    } else {
        render(&game);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn server_with_a_full_game() -> MockServer {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/games"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":42,"rootPlaceId":777,"name":"Sandbox Frontier",
                   "creator":{"id":1,"name":"Northwind Studio","type":"Group"},
                   "playing":1412,"visits":1540435,"favoritedCount":4678,"maxPlayers":16,
                   "created":"2026-07-03T17:35:39.839Z","updated":"2026-07-28T20:44:59Z",
                   "genre_l1":"Simulation","genre_l2":"Sandbox"}]}"#,
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/games/votes"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"data":[{"id":42,"upVotes":300,"downVotes":100}]}"#),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/games/icons"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"targetId":42,"state":"Completed","imageUrl":"https://cdn/icon"}]}"#,
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/games/multiget/thumbnails"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"universeId":42,"thumbnails":[
                   {"targetId":999,"state":"Completed","imageUrl":"https://cdn/banner"}]}]}"#,
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/games/42/media"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"assetTypeId":86,"assetType":"GamePreviewVideo","videoHash":"h"},
                   {"assetTypeId":1,"assetType":"Image","imageId":5}]}"#,
            ))
            .mount(&server)
            .await;

        server
    }

    #[tokio::test]
    async fn collect_joins_five_endpoints_into_one_report() {
        let server = server_with_a_full_game().await;
        // The place asset is read with the root place id from `detail`, not
        // the universe id — the guard in `api::economy` would refuse it.
        Mock::given(method("GET"))
            .and(path("/v2/assets/777/details"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    r#"{"AssetTypeId":9,"AssetId":777,"IconImageAssetId":123456}"#,
                ),
            )
            .expect(1)
            .mount(&server)
            .await;

        let game = collect(&Client::with_base_url(&server.uri()), 42)
            .await
            .unwrap();

        assert_eq!(game.detail.name, "Sandbox Frontier");
        assert_eq!(game.votes.up_votes, 300);
        assert_eq!(game.icon_asset_id, Some(123456));
        assert_eq!(game.images.banner_asset_id, Some(999));
        assert_eq!(game.media_count, 2);
        assert!(game.has_preview_video);
    }

    #[tokio::test]
    async fn an_unreadable_place_asset_degrades_instead_of_failing() {
        let server = server_with_a_full_game().await;
        Mock::given(method("GET"))
            .and(path("/v2/assets/777/details"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let game = collect(&Client::with_base_url(&server.uri()), 42)
            .await
            .unwrap();

        assert_eq!(game.icon_asset_id, None);
        assert_eq!(game.detail.name, "Sandbox Frontier");
    }

    #[test]
    fn approval_is_omitted_when_nobody_voted() {
        assert_eq!(approval(&Votes::default()), "");
        assert_eq!(
            approval(&Votes {
                up_votes: 3,
                down_votes: 1
            }),
            " (75% approval)"
        );
    }
}
