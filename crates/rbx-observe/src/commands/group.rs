//! `rbx-observe group` — a studio and its catalog.
//!
//! Games and the organisation that publishes them. There is no user-keyed twin
//! of this command: `games.roblox.com/v2/users/{userId}/games` exists and is
//! public, and a catalog keyed to an individual account is a person's output
//! rather than a studio's, which is the line this tool does not cross.

use std::collections::HashMap;

use std::fmt::Write;

use anyhow::Result;
use serde::Serialize;

use crate::api::groups::{Group, GroupGame};
use crate::api::Client;
use crate::render::{date, dim, heading, thousands, truncate};

#[derive(Debug, Serialize)]
pub struct GroupEntry {
    #[serde(flatten)]
    pub game: GroupGame,
    /// Players in the experience right now. `None` when the details endpoint
    /// had nothing for that universe, which happens on unlisted places.
    pub playing: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct GroupReport {
    pub group: Group,
    /// Public games only, unless the caller asked for everything.
    pub games: Vec<GroupEntry>,
    /// Players across the whole catalog right now.
    pub playing_now: u64,
    /// How many games Roblox returned that are **not** in the public listing:
    /// staging copies, test places, unreleased projects. Counted even when
    /// they are not listed, so the report never hides that they exist.
    pub unlisted_count: usize,
    pub total_visits: u64,
}

pub async fn collect(
    client: &Client,
    group_id: u64,
    include_unlisted: bool,
) -> Result<GroupReport> {
    let group = client.group(group_id).await?;
    let all = client.group_games(group_id).await?;

    let unlisted_count = all.iter().filter(|game| !game.public).count();
    let shown: Vec<GroupGame> = if include_unlisted {
        all
    } else {
        all.into_iter().filter(|game| game.public).collect()
    };

    // The catalog listing carries lifetime visits but not live players, and
    // the details endpoint takes 50 universe ids at a time — so the whole
    // catalog's CCU costs one request, not one per game.
    let universe_ids: Vec<u64> = shown.iter().map(|game| game.id).collect();
    let playing: HashMap<u64, u64> = client
        .game_details(&universe_ids)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|detail| (detail.id, detail.playing))
        .collect();

    let games: Vec<GroupEntry> = shown
        .into_iter()
        .map(|game| GroupEntry {
            playing: playing.get(&game.id).copied(),
            game,
        })
        .collect();

    Ok(GroupReport {
        // Visits and players of what is shown: adding a staging place's three
        // visits to a studio's lifetime total would be noise, not signal.
        total_visits: games.iter().map(|entry| entry.game.place_visits).sum(),
        playing_now: games.iter().filter_map(|entry| entry.playing).sum(),
        unlisted_count,
        group,
        games,
    })
}

pub fn render(report: &GroupReport) -> String {
    let mut out = String::new();
    let g = &report.group;

    let _ = writeln!(out, "{}", heading(&g.name));
    let _ = writeln!(
        out,
        "  group {} · {} members · {}",
        g.id,
        thousands(g.member_count),
        if g.public_entry_allowed {
            "open to join"
        } else {
            "approval required"
        }
    );
    if let Some(description) = g.description.as_deref().filter(|d| !d.is_empty()) {
        let _ = writeln!(out, "  {}", dim(&truncate(description, 300)));
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "{}", heading("Games"));
    if report.games.is_empty() {
        let _ = writeln!(out, "  {}", dim("none public"));
    }
    for entry in &report.games {
        let game = &entry.game;
        let marker = if game.public { "" } else { "  [not listed]" };
        let _ = writeln!(
            out,
            "  {:>14}  {}{}",
            thousands(game.place_visits),
            game.name,
            marker
        );
        let playing = match entry.playing {
            Some(count) => format!("{} playing now · ", thousands(count)),
            None => String::new(),
        };
        let _ = writeln!(
            out,
            "                  {}",
            dim(&format!(
                "{playing}universe {} · updated {} · rbx-observe game {}",
                game.id,
                date(game.updated.as_deref()),
                game.id
            ))
        );
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "{}", heading("Summary"));
    let _ = writeln!(
        out,
        "  {} game(s) shown · {} visits across them · {} playing now",
        report.games.len(),
        thousands(report.total_visits),
        thousands(report.playing_now)
    );
    if report.unlisted_count > 0 {
        let shown = report.games.iter().any(|entry| !entry.game.public);
        let _ = writeln!(
            out,
            "  {}",
            dim(&format!(
                "{} more are not in the public listing — staging and test places Roblox \
                 still returns anonymously{}",
                report.unlisted_count,
                if shown { "" } else { " (--all lists them)" }
            ))
        );
    }

    out
}

pub async fn run(client: &Client, group_id: u64, include_unlisted: bool, json: bool) -> Result<()> {
    let report = collect(client, group_id, include_unlisted).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", render(&report));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn studio_with_one_unlisted_game() -> MockServer {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/groups/7"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"id":7,"name":"Studio","memberCount":1200,"publicEntryAllowed":false,
                   "owner":{"userId":99,"username":"someone"}}"#,
            ))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v2/groups/7/gamesV2"))
            .and(query_param("accessFilter", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[
                   {"id":1,"name":"A","placeVisits":100},
                   {"id":2,"name":"B","placeVisits":900}],"nextPageCursor":null}"#,
            ))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v2/groups/7/gamesV2"))
            .and(query_param_is_missing("accessFilter"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[
                   {"id":1,"name":"A","placeVisits":100},
                   {"id":2,"name":"B","placeVisits":900},
                   {"id":3,"name":"[STAGING] B","placeVisits":3}],"nextPageCursor":null}"#,
            ))
            .mount(&server)
            .await;

        // Live players for the whole catalog in one batched call, not one per
        // game: `expect(1)` is what pins that.
        Mock::given(method("GET"))
            .and(path("/v1/games"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[
                   {"id":1,"rootPlaceId":11,"name":"A","playing":4},
                   {"id":2,"rootPlaceId":22,"name":"B","playing":96}]}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        server
    }

    #[tokio::test]
    async fn unlisted_games_are_counted_but_not_shown_by_default() {
        let server = studio_with_one_unlisted_game().await;

        let report = collect(&Client::with_base_url(&server.uri()), 7, false)
            .await
            .unwrap();

        assert_eq!(report.games.len(), 2);
        assert_eq!(report.unlisted_count, 1);
        assert_eq!(report.playing_now, 100);
        // Counted, not hidden — but its name is not printed unless asked.
        assert!(!report.games.iter().any(|entry| entry.game.id == 3));
        assert_eq!(report.total_visits, 1000);

        // The boundary again: the group's owner is in the payload and must not
        // reach the output.
        assert!(!serde_json::to_string(&report).unwrap().contains("owner"));
    }

    #[tokio::test]
    async fn all_includes_the_unlisted_ones() {
        let server = studio_with_one_unlisted_game().await;

        let report = collect(&Client::with_base_url(&server.uri()), 7, true)
            .await
            .unwrap();

        assert_eq!(report.games.len(), 3);
        assert_eq!(report.unlisted_count, 1);
        let staging = report
            .games
            .iter()
            .find(|entry| entry.game.id == 3)
            .unwrap();
        assert!(!staging.game.public);
    }

    #[tokio::test]
    async fn the_rendering_is_stable() {
        colored::control::set_override(false);
        let server = studio_with_one_unlisted_game().await;
        let report = collect(&Client::with_base_url(&server.uri()), 7, false)
            .await
            .unwrap();

        insta::assert_snapshot!(render(&report));
    }
}
