//! `groups.roblox.com` and the group half of `games.roblox.com` — a studio and
//! the games it publishes.
//!
//! # What is deliberately not read here
//!
//! The group payload also carries an `owner` object and a `shout` whose author
//! is a user, and the host offers `/v1/groups/{id}/roles` plus
//! `/v1/groups/{id}/roles/{roleId}/users` — the full membership roster, and
//! `/v2/users/{userId}/groups/roles` in the other direction. All of it is
//! public, none of it is here: those are person-level records, and this tool
//! reads studios and games, not people.
//!
//! The structs below simply do not have those fields. That is the enforcement:
//! serde drops what is not declared, so the data never enters the process.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::Client;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub member_count: u64,
    /// Whether anyone can join without approval. A studio group that is closed
    /// says something about how it recruits.
    #[serde(default)]
    pub public_entry_allowed: bool,
    #[serde(default)]
    pub has_verified_badge: bool,
}

/// The root place summary embedded in a group's game list.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RootPlace {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupGame {
    /// Universe id — feed it straight back to `rbx-observe game`.
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub root_place: RootPlace,
    #[serde(default)]
    pub place_visits: u64,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,

    /// Whether the game appears in the group's **public** listing.
    ///
    /// Not a field Roblox sends: it is derived by asking twice, once with
    /// `accessFilter=2` and once without, and comparing. See `group_games`.
    #[serde(skip_deserializing, default = "yes")]
    pub public: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GamePage {
    #[serde(default = "Vec::new")]
    data: Vec<GroupGame>,
    #[serde(default)]
    next_page_cursor: Option<String>,
}

const PAGE_SIZE: &str = "100";
const MAX_PAGES: usize = 50;

/// `2` is "Public". The filter exists because the endpoint otherwise mixes in
/// entries the caller is not entitled to see, which anonymously means empty
/// husks rather than games.
const ACCESS_PUBLIC: &str = "2";

impl Client {
    pub async fn group(&self, group_id: u64) -> Result<Group> {
        let url = format!("{}/v1/groups/{group_id}", self.hosts().groups);
        self.get_json(&url, &[]).await
    }

    /// Every game the group has, each marked public or not, most-visited
    /// first.
    ///
    /// **The endpoint hands an anonymous caller the group's unlisted work.**
    /// Without `accessFilter`, or with `accessFilter=1`, the response includes
    /// staging copies, test places and unreleased projects — one measured
    /// group answers 2 games filtered to public and 21 unfiltered, the extra
    /// nineteen being `[STAGING]`, `[TESTING]`, `AA_TEST` and friends. Roblox
    /// serves it; that does not make it something to print by surprise.
    ///
    /// So the listing is asked for twice and compared, which is what allows
    /// the caller to say "and 19 more that are not publicly listed" without
    /// naming them unless asked.
    ///
    /// `gamesV2` rather than `games`: the v1 shape omits `rootPlace`, which is
    /// what turns a listing into something you can open.
    pub async fn group_games(&self, group_id: u64) -> Result<Vec<GroupGame>> {
        let public: Vec<GroupGame> = self.group_games_page(group_id, true).await?;
        let mut all: Vec<GroupGame> = self.group_games_page(group_id, false).await?;

        let public_ids: std::collections::HashSet<u64> =
            public.iter().map(|game| game.id).collect();
        for game in &mut all {
            game.public = public_ids.contains(&game.id);
        }

        all.sort_by(|a, b| {
            b.place_visits
                .cmp(&a.place_visits)
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(all)
    }

    async fn group_games_page(&self, group_id: u64, public_only: bool) -> Result<Vec<GroupGame>> {
        let url = format!("{}/v2/groups/{group_id}/gamesV2", self.hosts().games);
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;

        for _ in 0..MAX_PAGES {
            let mut params: Vec<(&str, &str)> = vec![("limit", PAGE_SIZE), ("sortOrder", "Asc")];
            if public_only {
                params.push(("accessFilter", ACCESS_PUBLIC));
            }
            if let Some(value) = cursor.as_deref() {
                params.push(("cursor", value));
            }

            let page: GamePage = self.get_json(&url, &params).await?;
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
    use wiremock::matchers::{method, path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn the_group_is_read_without_its_owner_or_its_shout() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/groups/33333333333"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"id":33333333333,"name":"Northwind Studio","description":"main group",
                   "owner":{"userId":6666666666666,"username":"someone","displayName":"Someone"},
                   "shout":{"body":"hi","poster":{"userId":6666666666666,"username":"someone"}},
                   "memberCount":49617,"publicEntryAllowed":true,"hasVerifiedBadge":false}"#,
            ))
            .mount(&server)
            .await;

        let group = Client::with_base_url(&server.uri())
            .group(33333333333)
            .await
            .unwrap();

        assert_eq!(group.name, "Northwind Studio");
        assert_eq!(group.member_count, 49617);

        // The boundary, asserted rather than promised: the owner and the
        // shout's poster are in the payload and must not survive parsing.
        let round_tripped = serde_json::to_string(&group).unwrap();
        assert!(!round_tripped.contains("owner"), "{round_tripped}");
        assert!(!round_tripped.contains("6666666666666"), "{round_tripped}");
        assert!(!round_tripped.contains("shout"), "{round_tripped}");
    }

    #[tokio::test]
    async fn unlisted_games_are_returned_but_marked_as_such() {
        let server = MockServer::start().await;

        // Public listing: two games, paginated to prove the cursor is walked
        // on this call too.
        Mock::given(method("GET"))
            .and(path("/v2/groups/33333333333/gamesV2"))
            .and(query_param("accessFilter", "2"))
            .and(query_param("limit", "100"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[
                   {"id":1,"name":"Small","placeVisits":10,"rootPlace":{"id":11,"name":"Small"}},
                   {"id":2,"name":"Big","placeVisits":9000,"rootPlace":{"id":22,"name":"Big"}}],
                   "nextPageCursor":"c2"}"#,
            ))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v2/groups/33333333333/gamesV2"))
            .and(query_param("accessFilter", "2"))
            .and(query_param("cursor", "c2"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"data":[],"nextPageCursor":null}"#),
            )
            .mount(&server)
            .await;

        // Unfiltered listing: the same two, plus a staging place the group
        // never published. This is what Roblox hands an anonymous caller.
        Mock::given(method("GET"))
            .and(path("/v2/groups/33333333333/gamesV2"))
            .and(query_param_is_missing("accessFilter"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[
                   {"id":1,"name":"Small","placeVisits":10,"rootPlace":{"id":11,"name":"Small"}},
                   {"id":2,"name":"Big","placeVisits":9000,"rootPlace":{"id":22,"name":"Big"}},
                   {"id":3,"name":"[STAGING] Big","placeVisits":3}],"nextPageCursor":null}"#,
            ))
            .mount(&server)
            .await;

        let games = Client::with_base_url(&server.uri())
            .group_games(33333333333)
            .await
            .unwrap();

        assert_eq!(games.len(), 3);
        assert_eq!(games[0].name, "Big");
        assert!(games[0].public);
        assert_eq!(games[0].root_place.id, Some(22));

        let staging = games.iter().find(|game| game.id == 3).unwrap();
        assert!(!staging.public, "a game absent from the public listing");
    }
}
