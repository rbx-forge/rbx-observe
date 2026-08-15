//! `rbx-observe charts` — what Roblox is currently pushing.
//!
//! The discovery command: every other one needs a universe id, this one hands
//! them out. `top-earning` is the interesting column — a revenue proxy Roblox
//! publishes for free — and the `trending-in-<category>` sorts are its real
//! genre taxonomy.

use anyhow::{bail, Result};
use serde::Serialize;

use crate::api::explore::Sort;
use crate::api::Client;
use crate::render::{dim, heading, thousands};

#[derive(Debug, Serialize)]
pub struct Charts {
    pub sorts: Vec<Sort>,
    /// Rows kept per sort in the human rendering. `--json` is never truncated.
    pub limit: usize,
}

pub async fn collect(
    client: &Client,
    sort_filter: Option<&str>,
    category: Option<&str>,
    limit: usize,
) -> Result<Charts> {
    let mut sorts = client.sorts().await?;

    if let Some(wanted) = sort_filter {
        sorts.retain(|sort| sort.sort_id == wanted);
        if sorts.is_empty() {
            bail!(
                "No sort called {wanted:?}. Run `rbx-observe charts` with no --sort to see \
                 the ones Roblox is currently serving."
            );
        }
    }

    // Roblox's real genre taxonomy lives in the sort ids: the fourteen
    // `trending-in-<category>` rankings say more than the `genre` field on a
    // game, which reads "All" almost everywhere.
    if let Some(wanted) = category {
        sorts.retain(|sort| sort.category() == Some(wanted));
        if sorts.is_empty() {
            bail!(
                "No category called {wanted:?}. The categories are the suffixes of the \
                 `trending-in-…` sort ids, which `rbx-observe charts` lists."
            );
        }
    }

    Ok(Charts { sorts, limit })
}

/// Same column width as the storefront and badge listings, so the three read
/// as one tool.
const NAME_WIDTH: usize = 34;

pub fn render(charts: &Charts) {
    for sort in &charts.sorts {
        let title = sort.sort_display_name.as_deref().unwrap_or(&sort.sort_id);
        println!("{}", heading(title));
        println!(
            "  {}",
            dim(&format!("{} · {} games", sort.sort_id, sort.games.len()))
        );

        for (index, game) in sort.games.iter().take(charts.limit).enumerate() {
            let sponsored = if game.is_sponsored {
                "  [sponsored]"
            } else {
                ""
            };
            println!(
                "  {:>3}. {:>9}  {:<NAME_WIDTH$}  {}",
                index + 1,
                thousands(game.player_count),
                format!("{}{}", game.name, sponsored),
                dim(&format!(
                    "universe {} · {}{}",
                    game.universe_id,
                    game.genre_l1.as_deref().unwrap_or("-"),
                    match game.minimum_age {
                        Some(age) if age > 0 => format!(" · {age}+"),
                        _ => String::new(),
                    },
                ))
            );
        }

        if sort.games.len() > charts.limit {
            println!(
                "  {}",
                dim(&format!(
                    "{} more in this sort (--limit)",
                    sort.games.len() - charts.limit
                ))
            );
        }
        println!();
    }

    println!(
        "  {}",
        dim("player counts are Roblox's own, as shown on the discovery page")
    );
    println!("  {}", dim("drill in with  rbx-observe game <universe id>"));
}

pub async fn run(
    client: &Client,
    sort_filter: Option<&str>,
    category: Option<&str>,
    limit: usize,
    json: bool,
) -> Result<()> {
    let charts = collect(client, sort_filter, category, limit).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&charts)?);
    } else {
        render(&charts);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn two_sorts() -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/explore-api/v1/get-sorts"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"sorts":[
                   {"sortId":"top-playing-now","contentType":"Games","sortDisplayName":"Top Playing Now",
                    "games":[{"universeId":1,"name":"A","playerCount":900},
                             {"universeId":2,"name":"B","playerCount":100}]},
                   {"sortId":"top-earning","contentType":"Games","sortDisplayName":"Top Earning",
                    "games":[{"universeId":3,"name":"C","playerCount":50}]}],
                   "nextSortsPageToken":null}"#,
            ))
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn every_sort_is_returned_when_no_filter_is_given() {
        let server = two_sorts().await;
        let charts = collect(&Client::with_base_url(&server.uri()), None, None, 10)
            .await
            .unwrap();

        assert_eq!(charts.sorts.len(), 2);
        assert_eq!(charts.sorts[0].games.len(), 2);
    }

    #[tokio::test]
    async fn a_filter_keeps_one_sort_and_an_unknown_one_names_the_fix() {
        let server = two_sorts().await;
        let client = Client::with_base_url(&server.uri());

        let charts = collect(&client, Some("top-earning"), None, 10)
            .await
            .unwrap();
        assert_eq!(charts.sorts.len(), 1);
        assert_eq!(charts.sorts[0].games[0].name, "C");

        let error = collect(&client, Some("nope"), None, 10)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("--sort"), "{error}");
    }

    #[tokio::test]
    async fn a_category_filter_matches_the_trending_sort_suffix() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/explore-api/v1/get-sorts"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"sorts":[
                   {"sortId":"top-playing-now","contentType":"Games",
                    "games":[{"universeId":1,"name":"A","playerCount":9}]},
                   {"sortId":"trending-in-obby-and-platformer","contentType":"Games",
                    "games":[{"universeId":2,"name":"Tower","playerCount":5}]}],
                   "nextSortsPageToken":null}"#,
            ))
            .mount(&server)
            .await;
        let client = Client::with_base_url(&server.uri());

        let charts = collect(&client, None, Some("obby-and-platformer"), 10)
            .await
            .unwrap();
        assert_eq!(charts.sorts.len(), 1);
        assert_eq!(charts.sorts[0].games[0].name, "Tower");

        let error = collect(&client, None, Some("nope"), 10)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("trending-in"), "{error}");
    }

    #[tokio::test]
    async fn the_limit_truncates_the_rendering_not_the_data() {
        let server = two_sorts().await;
        let charts = collect(&Client::with_base_url(&server.uri()), None, None, 1)
            .await
            .unwrap();

        // `--json` has to stay complete: the limit is a display concern, and a
        // truncated document would be a lie about what Roblox returned.
        assert_eq!(charts.sorts[0].games.len(), 2);
        assert_eq!(charts.limit, 1);
    }
}
