//! `rbx-observe game` — what the experience's page says about itself.

use std::fmt::Write;

use anyhow::Result;
use serde::Serialize;

use crate::api::games::{GameDetail, Votes};
use crate::api::maturity::Maturity;
use crate::api::places::Place;
use crate::api::thumbnails::Images;
use crate::api::Client;
use crate::render::{asset_hint, block, date, dim, heading, thousands, truncate};

#[derive(Debug, Serialize)]
pub struct Game {
    pub detail: GameDetail,
    pub votes: Votes,
    pub images: Images,
    pub maturity: Maturity,
    /// Every place in the universe, not just the root one the page shows.
    pub places: Vec<Place>,
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

    // Both of these are extras rather than the point of the command, and both
    // sit on hosts that answer for a narrower set of experiences than the
    // games endpoint does. A failure degrades one section instead of losing
    // the report.
    let maturity = client.maturity(universe_id).await.unwrap_or_default();
    let places = client
        .universe_places(universe_id)
        .await
        .unwrap_or_default();

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
        maturity,
        places,
    })
}

pub fn render(game: &Game) -> String {
    let mut out = String::new();
    let d = &game.detail;

    let _ = writeln!(out, "{}", heading(&d.name));
    let _ = writeln!(out, "  universe {} · place {}", d.id, d.root_place_id);
    let _ = writeln!(out, "  {}", creator_line(&d.creator));
    let _ = writeln!(out, "  https://www.roblox.com/games/{}", d.root_place_id);
    let _ = writeln!(out);

    if let Some(description) = d.description.as_deref().filter(|text| !text.is_empty()) {
        let _ = writeln!(out, "{}", heading("Description"));
        let _ = writeln!(out, "{}", dim(&block(&truncate(description, 600), "  ")));
        let _ = writeln!(out);
    }

    let _ = writeln!(out, "{}", heading("Audience"));
    let _ = writeln!(out, "  playing now   {}", thousands(d.playing));
    let _ = writeln!(out, "  visits        {}", thousands(d.visits));
    let _ = writeln!(out, "  favorites     {}", thousands(d.favorited_count));
    let _ = writeln!(
        out,
        "  votes         {} up / {} down{}",
        thousands(game.votes.up_votes),
        thousands(game.votes.down_votes),
        approval(&game.votes)
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "{}", heading("Shape"));
    let _ = writeln!(out, "  max players   {}", d.max_players);
    let _ = writeln!(out, "  genre         {}", genre(d));
    let _ = writeln!(out, "  maturity      {}", maturity_line(&game.maturity));
    let _ = writeln!(out, "  age           {}", age_line(&game.maturity));
    let _ = writeln!(out, "  created       {}", date(d.created.as_deref()));
    let _ = writeln!(out, "  updated       {}", date(d.updated.as_deref()));
    if let Some(price) = d.price {
        let _ = writeln!(out, "  paid access   R$ {}", thousands(price));
    }
    for descriptor in &game.maturity.descriptors {
        if let Some(name) = descriptor.display_name.as_deref() {
            let _ = writeln!(out, "  {}", dim(&format!("contains: {name}")));
        }
    }
    let _ = writeln!(out);

    if !game.places.is_empty() {
        let _ = writeln!(out, "{}", heading("Places"));
        for place in &game.places {
            let role = if place.id == d.root_place_id {
                " (root)"
            } else {
                ""
            };
            let _ = writeln!(out, "  {}  {}{}", place.id, place.name, role);
        }
        let _ = writeln!(
            out,
            "  {}",
            dim(&format!(
                "rbx-observe places {} for published state and live servers",
                d.id
            ))
        );
        let _ = writeln!(out);
    }

    let _ = writeln!(out, "{}", heading("Assets"));
    match game.icon_asset_id {
        Some(id) => {
            let _ = writeln!(out, "  icon          {id}");
        }
        None => {
            let _ = writeln!(out, "  icon          {}", dim("asset id unavailable"));
        }
    }
    match game.images.banner_asset_id {
        Some(id) => {
            let _ = writeln!(out, "  banner        {id}");
        }
        None => {
            let _ = writeln!(out, "  banner        {}", dim("none"));
        }
    }
    let _ = writeln!(
        out,
        "  carousel      {} item(s), {}",
        game.media_count,
        if game.has_preview_video {
            "opens on a video"
        } else {
            "no preview video"
        }
    );

    let ids: Vec<u64> = [game.icon_asset_id, game.images.banner_asset_id]
        .into_iter()
        .flatten()
        .collect();
    if let Some(hint) = asset_hint(&ids) {
        let _ = writeln!(out, "  {}", dim(&hint));
    }

    out
}

/// Roblox's own taxonomy first: the legacy `genre` field says "All" on the
/// large majority of experiences, so it is the fallback, not the answer.
fn genre(detail: &GameDetail) -> String {
    // Roblox sends an empty string rather than null for a missing subgenre, so
    // `Option` alone would print "Simulation / " on every game without one.
    let present = |field: &Option<String>| {
        field
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };

    match (present(&detail.genre_l1), present(&detail.genre_l2)) {
        (Some(l1), Some(l2)) => format!("{l1} / {l2}"),
        (Some(l1), None) => l1,
        _ => present(&detail.genre).unwrap_or_else(|| "-".to_string()),
    }
}

/// Publisher attribution: who the game page says published this, with the id
/// and the kind of account it is.
///
/// For a `Group` creator, `creator.id` is the group id itself, so the drill-
/// down is one command away. For a `User` creator there is no equivalent here
/// on purpose: a catalog keyed to an individual account is a person's output.
fn creator_line(creator: &crate::api::games::Creator) -> String {
    let name = creator.name.as_deref().unwrap_or("unknown");
    let kind = creator.kind.as_deref().unwrap_or("creator");

    match (creator.id, creator.kind.as_deref()) {
        (Some(id), Some("Group")) => {
            format!("{kind} {name} ({id}) · rbx-observe group {id}")
        }
        (Some(id), _) => format!("{kind} {name} ({id})"),
        (None, _) => format!("{kind} {name}"),
    }
}

/// The content label alone: `Minimal`, `Mild`, `Moderate`, `Restricted`, or
/// `-` while Roblox has not rated the experience.
fn maturity_line(maturity: &Maturity) -> String {
    maturity
        .recommendation
        .display_name
        .clone()
        .unwrap_or_else(|| "-".to_string())
}

/// The age gate, which is a different axis from the maturity label: two
/// experiences can both be `Minimal` while one is open to everyone and the
/// other is 16+.
///
/// `0` means no gate, and saying "all ages" is the useful rendering of that.
/// It is distinct from `-`, which means Roblox has not rated the experience
/// at all and therefore has not said anything about age either.
fn age_line(maturity: &Maturity) -> String {
    let recommendation = &maturity.recommendation;

    if let Some(display) = recommendation
        .minimum_age_display
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return display.to_string();
    }

    match recommendation.minimum_age {
        Some(0) => "all ages".to_string(),
        Some(age) => format!("{age}+"),
        // No age at all, but a label: rated, and the gate is simply absent.
        None if recommendation.display_name.is_some() => "all ages".to_string(),
        None => "-".to_string(),
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
        print!("{}", render(&game));
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

    #[tokio::test]
    async fn maturity_and_places_enrich_the_report_when_the_hosts_answer() {
        let server = server_with_a_full_game().await;
        Mock::given(method("GET"))
            .and(path("/v2/assets/777/details"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    r#"{"AssetTypeId":9,"AssetId":777,"IconImageAssetId":123456}"#,
                ),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(
                "/experience-guidelines-api/experience-guidelines/get-age-recommendation",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"ageRecommendationDetails":{"summary":{"ageRecommendation":
                   {"displayName":"Mild","minimumAge":9}},"descriptorUsages":[]}}"#,
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/universes/42/places"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":777,"name":"Main"},{"id":778,"name":"Tutorial"}],
                   "nextPageCursor":null}"#,
            ))
            .mount(&server)
            .await;

        let game = collect(&Client::with_base_url(&server.uri()), 42)
            .await
            .unwrap();

        assert_eq!(maturity_line(&game.maturity), "Mild");
        assert_eq!(age_line(&game.maturity), "9+");
        assert_eq!(game.places.len(), 2);
    }

    #[tokio::test]
    async fn maturity_and_places_are_optional_sections() {
        // Neither host is mocked at all here: both calls fail, and the report
        // still has to come back rather than taking the command down with it.
        let server = server_with_a_full_game().await;
        Mock::given(method("GET"))
            .and(path("/v2/assets/777/details"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let game = collect(&Client::with_base_url(&server.uri()), 42)
            .await
            .unwrap();

        assert_eq!(maturity_line(&game.maturity), "-");
        assert!(game.places.is_empty());
        assert_eq!(game.detail.name, "Sandbox Frontier");
    }

    #[test]
    fn an_empty_subgenre_does_not_leave_a_dangling_separator() {
        let detail = |l1: &str, l2: &str, legacy: &str| GameDetail {
            id: 1,
            root_place_id: 2,
            name: String::new(),
            description: None,
            creator: Default::default(),
            price: None,
            playing: 0,
            visits: 0,
            max_players: 0,
            favorited_count: 0,
            created: None,
            updated: None,
            genre: Some(legacy.to_string()),
            genre_l1: Some(l1.to_string()),
            genre_l2: Some(l2.to_string()),
        };

        // Roblox sends "" rather than null here, which is what produced
        // "Simulation / " with a trailing separator.
        assert_eq!(genre(&detail("Simulation", "", "All")), "Simulation");
        assert_eq!(
            genre(&detail("Simulation", "Sandbox", "All")),
            "Simulation / Sandbox"
        );
        assert_eq!(genre(&detail("", "", "All")), "All");
        assert_eq!(genre(&detail("", "", "")), "-");
    }

    #[test]
    fn the_creator_line_carries_the_id_and_the_kind() {
        use crate::api::games::Creator;

        let group = Creator {
            id: Some(33333333334),
            name: Some("Northwind Studio".into()),
            kind: Some("Group".into()),
        };
        assert_eq!(
            creator_line(&group),
            "Group Northwind Studio (33333333334) · rbx-observe group 33333333334"
        );

        // A user creator gets the same attribution and no drill-down: the
        // user-keyed catalog endpoint is the one this tool does not call.
        let user = Creator {
            id: Some(99),
            name: Some("someone".into()),
            kind: Some("User".into()),
        };
        assert_eq!(creator_line(&user), "User someone (99)");

        assert_eq!(creator_line(&Creator::default()), "creator unknown");
    }

    #[test]
    fn the_age_gate_and_the_maturity_label_are_reported_separately() {
        let rated = |age: Option<u32>, display: Option<&str>| {
            let mut maturity = Maturity::default();
            maturity.recommendation.display_name = Some("Minimal".to_string());
            maturity.recommendation.minimum_age = age;
            maturity.recommendation.minimum_age_display = display.map(str::to_string);
            maturity
        };

        // Both of these are `Minimal`. Only the gate differs, which is the
        // whole reason they are two lines: recorded from a 16+ experience and
        // an all-ages one that share a maturity label.
        let gated = rated(Some(16), Some("16+"));
        assert_eq!(maturity_line(&gated), "Minimal");
        assert_eq!(age_line(&gated), "16+");

        let open = rated(Some(0), Some(""));
        assert_eq!(maturity_line(&open), "Minimal");
        assert_eq!(age_line(&open), "all ages");

        // Unrated is not the same as unrestricted, and must not read as one.
        assert_eq!(maturity_line(&Maturity::default()), "-");
        assert_eq!(age_line(&Maturity::default()), "-");
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

    #[tokio::test]
    async fn the_rendering_is_stable() {
        colored::control::set_override(false);
        let server = server_with_a_full_game().await;
        Mock::given(method("GET"))
            .and(path("/v2/assets/777/details"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    r#"{"AssetTypeId":9,"AssetId":777,"IconImageAssetId":123456}"#,
                ),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path(
                "/experience-guidelines-api/experience-guidelines/get-age-recommendation",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"ageRecommendationDetails":{"summary":{"ageRecommendation":
                   {"displayName":"Minimal","minimumAge":16,"minimumAgeDisplay":"16+"}},
                   "descriptorUsages":[{"descriptor":{"displayName":"Violence (Mild)"},
                                        "contains":true}]}}"#,
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/universes/42/places"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"id":777,"name":"Main"},{"id":778,"name":"Tutorial"}],
                   "nextPageCursor":null}"#,
            ))
            .mount(&server)
            .await;

        let game = collect(&Client::with_base_url(&server.uri()), 42)
            .await
            .unwrap();

        insta::assert_snapshot!(render(&game));
    }
}
