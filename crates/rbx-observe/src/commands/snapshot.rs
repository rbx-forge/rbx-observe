//! `rbx-observe snapshot` — one experience, one document, one moment.
//!
//! The other commands each answer a question. This one produces the artifact:
//! everything about a game as of now, in a single JSON document worth keeping
//! on disk. Comparing a storefront across weeks is the point of the tool, and
//! until this existed it meant running five commands and reconciling five
//! files by hand.
//!
//! It is not five commands glued together. `game` already fetches the
//! carousel and the place list, so this reuses them instead of asking twice —
//! three requests saved out of roughly fifteen, which is small, but asking
//! Roblox for an answer you are already holding is the kind of thing that adds
//! up across a category sweep.

use std::fmt::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde::Serialize;

use crate::api::Client;
use crate::commands::{badges, game, media, places, storefront};
use crate::render::{dim, heading, thousands};

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub universe_id: u64,
    /// Seconds since the epoch. Deliberately not a formatted date: the file
    /// name is where a human date belongs, and a snapshot that disagrees with
    /// its own file name is worse than one that carries a number.
    pub captured_at_unix: u64,
    pub game: game::Game,
    pub storefront: storefront::Storefront,
    pub badges: badges::Badges,
    /// Present only with `--places`: two requests per place, none of which
    /// batch, so a twenty-place universe triples the time this takes.
    pub places: Option<places::Places>,
}

pub async fn collect(client: &Client, universe_id: u64, with_places: bool) -> Result<Snapshot> {
    let game = game::collect(client, universe_id).await?;
    let storefront = storefront::collect(client, universe_id).await?;
    let badges = badges::collect(client, universe_id).await?;

    let places = if with_places {
        places::collect_with(
            client,
            universe_id,
            game.detail.root_place_id,
            game.places.clone(),
        )
        .await
        .ok()
    } else {
        None
    };

    Ok(Snapshot {
        universe_id,
        captured_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or_default(),
        game,
        storefront,
        badges,
        places,
    })
}

/// Everything, in reading order: what the game is, what it sells, what it
/// rewards, what it shows, and — when asked for — what runs on each place.
///
/// This composes the per-command renderings instead of reimplementing them,
/// so `snapshot` and `storefront` cannot drift into printing one catalogue
/// two different ways.
pub fn render(snapshot: &Snapshot) -> String {
    let mut out = String::new();

    // A blank line between sections: each `render` ends with its own content
    // and no trailing gap, so the separation belongs to whoever composes them.
    let mut section = |rendered: String| {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&rendered);
    };

    section(game::render(&snapshot.game));
    section(storefront::render(&snapshot.storefront));
    section(badges::render(&snapshot.badges));
    section(media::render(&media::from_entries(
        snapshot.universe_id,
        snapshot.game.media.clone(),
    )));
    if let Some(places) = &snapshot.places {
        section(places::render(places));
    }
    section(render_summary(snapshot));
    out
}

/// The same experience in a dozen lines, for the moment somebody wants its
/// shape rather than its detail.
pub fn render_summary(snapshot: &Snapshot) -> String {
    let mut out = String::new();
    let d = &snapshot.game.detail;
    let s = &snapshot.storefront.summary;
    let b = &snapshot.badges.summary;

    let _ = writeln!(out, "{}", heading("At a glance"));
    let _ = writeln!(out, "  {}", d.name);
    let _ = writeln!(out, "  universe {} · place {}", d.id, d.root_place_id);
    let _ = writeln!(out);

    let _ = writeln!(out, "  playing now   {}", thousands(d.playing));
    let _ = writeln!(out, "  visits        {}", thousands(d.visits));
    let _ = writeln!(out, "  favorites     {}", thousands(d.favorited_count));
    let _ = writeln!(
        out,
        "  votes         {} up / {} down",
        thousands(snapshot.game.votes.up_votes),
        thousands(snapshot.game.votes.down_votes)
    );
    let _ = writeln!(
        out,
        "  storefront    {} pass(es), {} product(s) on sale",
        s.passes_for_sale, s.products_for_sale
    );
    if let (Some(min), Some(median), Some(max)) = (s.min_price, s.median_price, s.max_price) {
        let _ = writeln!(
            out,
            "  prices        R$ {} low · R$ {} median · R$ {} high",
            thousands(min),
            thousands(median),
            thousands(max)
        );
    }
    let _ = writeln!(
        out,
        "  badges        {} ({} awarded, {} in the last day)",
        b.total,
        thousands(b.awarded_total),
        thousands(b.awarded_past_day)
    );
    let _ = writeln!(out, "  places        {}", snapshot.game.places.len());
    let _ = writeln!(
        out,
        "  carousel      {} item(s), {}",
        snapshot.game.media.len(),
        if snapshot.game.has_preview_video {
            "opens on a video"
        } else {
            "no preview video"
        }
    );
    let _ = writeln!(out);

    let _ = writeln!(
        out,
        "  {}",
        dim("--json carries every field behind these numbers")
    );

    out
}

pub async fn run(
    client: &Client,
    universe_id: u64,
    with_places: bool,
    summary: bool,
    json: bool,
) -> Result<()> {
    let snapshot = collect(client, universe_id, with_places).await?;
    match (json, summary) {
        (true, _) => println!("{}", serde_json::to_string_pretty(&snapshot)?),
        (false, true) => print!("{}", render_summary(&snapshot)),
        (false, false) => print!("{}", render(&snapshot)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn one_experience() -> MockServer {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/games"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":1111111111111,"rootPlaceId":2222222222222221,"name":"A Game",
                   "creator":{"id":33333333333,"name":"A Studio","type":"Group"},
                   "playing":120,"visits":45000,"favoritedCount":900,"maxPlayers":16}]}"#,
            ))
            // The point of the command: `game` fetched the details, and no
            // other section asks for them again.
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/games/votes"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    r#"{"data":[{"id":1111111111111,"upVotes":80,"downVotes":20}]}"#,
                ),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/games/icons"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/games/multiget/thumbnails"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":[]}"#))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/games/1111111111111/media"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"assetTypeId":1,"assetType":"Image","imageId":4444444444444441}]}"#,
            ))
            // Same again: `media` is read once, by `game`.
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/universes/1111111111111/places"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":2222222222222221,"name":"Main"}],"nextPageCursor":null}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/assets/2222222222222221/details"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"AssetTypeId":9,"AssetId":2222222222222221,"ProductId":5555555555555,
                   "IconImageAssetId":4444444444444442}"#,
            ))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(
                "/experience-guidelines-api/experience-guidelines/get-age-recommendation",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"ageRecommendationDetails":{"summary":{"ageRecommendation":
                   {"displayName":"Minimal","minimumAge":0}},"descriptorUsages":[]}}"#,
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/game-passes/v1/universes/1111111111111/game-passes"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"gamePasses":[{"id":5555555555551,"name":"VIP","price":399,
                   "isForSale":true,"displayIconImageAssetId":4444444444444443}],
                   "nextPageToken":null}"#,
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(
                "/developer-products/v2/universes/1111111111111/developerproducts",
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"{"developerProducts":[],"nextPageCursor":null}"#),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/universes/1111111111111/badges"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":1,"name":"Welcome","enabled":true,
                   "statistics":{"pastDayAwardedCount":30,"awardedCount":5000,
                                 "winRatePercentage":0.8}}],"nextPageCursor":null}"#,
            ))
            .mount(&server)
            .await;

        server
    }

    #[tokio::test]
    async fn one_document_holds_every_section_and_asks_once_for_each_answer() {
        let server = one_experience().await;

        let snapshot = collect(&Client::with_base_url(&server.uri()), 1111111111111, false)
            .await
            .unwrap();

        assert_eq!(snapshot.game.detail.name, "A Game");
        assert_eq!(snapshot.storefront.summary.passes_for_sale, 1);
        assert_eq!(snapshot.badges.summary.awarded_total, 5000);
        assert_eq!(snapshot.game.media.len(), 1);
        assert!(snapshot.places.is_none(), "not asked for");
        assert!(snapshot.captured_at_unix > 1_700_000_000);
        // `expect(1)` on the shared endpoints is the real assertion here:
        // wiremock fails on drop if `game` was not the only caller.
    }

    #[tokio::test]
    async fn places_are_included_on_request_without_refetching_the_list() {
        let server = one_experience().await;
        Mock::given(method("GET"))
            .and(path("/v1/games/2222222222222221/servers/Public"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"maxPlayers":16,"playing":12}],"nextPageCursor":null}"#,
            ))
            .mount(&server)
            .await;

        let snapshot = collect(&Client::with_base_url(&server.uri()), 1111111111111, true)
            .await
            .unwrap();

        let places = snapshot.places.expect("asked for");
        assert_eq!(places.places.len(), 1);
        assert!(places.places[0].is_root);
        assert_eq!(places.places[0].servers.playing, 12);
    }

    #[tokio::test]
    async fn the_rendering_is_stable() {
        colored::control::set_override(false);
        let server = one_experience().await;
        let mut snapshot = collect(&Client::with_base_url(&server.uri()), 1111111111111, false)
            .await
            .unwrap();
        // The clock is the one thing that cannot be snapshotted.
        snapshot.captured_at_unix = 1_770_000_000;

        insta::assert_snapshot!(render(&snapshot));
    }

    #[tokio::test]
    async fn the_summary_rendering_is_stable() {
        colored::control::set_override(false);
        let server = one_experience().await;
        let mut snapshot = collect(&Client::with_base_url(&server.uri()), 1111111111111, false)
            .await
            .unwrap();
        snapshot.captured_at_unix = 1_770_000_000;

        insta::assert_snapshot!(render_summary(&snapshot));
    }
}
