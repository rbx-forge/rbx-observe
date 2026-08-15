//! `rbx-observe badges` — what the experience rewards, and how often.

use anyhow::Result;
use serde::Serialize;

use crate::api::badges::Badge;
use crate::api::Client;
use crate::render::{asset_hint, dim, heading, thousands};

#[derive(Debug, Serialize)]
pub struct Badges {
    pub universe_id: u64,
    pub badges: Vec<Badge>,
    pub summary: Summary,
}

#[derive(Debug, Default, Serialize)]
pub struct Summary {
    pub total: usize,
    pub enabled: usize,
    /// Awards across all badges in the last 24 hours. The one number here that
    /// moves daily, which makes it the one worth trending.
    pub awarded_past_day: u64,
    pub awarded_total: u64,
}

impl Summary {
    fn of(badges: &[Badge]) -> Self {
        Self {
            total: badges.len(),
            enabled: badges.iter().filter(|b| b.enabled).count(),
            awarded_past_day: badges
                .iter()
                .map(|b| b.statistics.past_day_awarded_count)
                .sum(),
            awarded_total: badges.iter().map(|b| b.statistics.awarded_count).sum(),
        }
    }
}

pub async fn collect(client: &Client, universe_id: u64) -> Result<Badges> {
    let mut badges = client.badges(universe_id).await?;

    // Most awarded first: on a game with forty badges, the tail is onboarding
    // noise and the head is what players actually reach.
    badges.sort_by(|a, b| {
        b.statistics
            .awarded_count
            .cmp(&a.statistics.awarded_count)
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(Badges {
        universe_id,
        summary: Summary::of(&badges),
        badges,
    })
}

/// Width of the name column before the dim stats block. Matches the
/// storefront so the two commands read as one tool.
const NAME_WIDTH: usize = 30;

pub fn render(report: &Badges) {
    println!("{}", heading("Badges"));
    if report.badges.is_empty() {
        println!("  {}", dim("none"));
    }

    for badge in &report.badges {
        let state = if badge.enabled { "" } else { " (disabled)" };
        println!(
            "  {:>12}  {:<NAME_WIDTH$}  {}",
            thousands(badge.statistics.awarded_count),
            format!("{}{}", badge.name, state),
            dim(&format!(
                "+{} today · {:.0}% win · icon {}",
                thousands(badge.statistics.past_day_awarded_count),
                badge.statistics.win_rate_percentage * 100.0,
                match badge.icon_image_id {
                    Some(id) => id.to_string(),
                    None => "none".to_string(),
                }
            ))
        );
    }
    println!();

    let s = &report.summary;
    println!("{}", heading("Summary"));
    println!("  {} badge(s), {} enabled", s.total, s.enabled);
    println!(
        "  {} awarded in total · {} in the last day",
        thousands(s.awarded_total),
        thousands(s.awarded_past_day)
    );

    let icons: Vec<u64> = report
        .badges
        .iter()
        .filter_map(|badge| badge.icon_image_id)
        .collect();
    if let Some(hint) = asset_hint(&icons) {
        println!("  {}", dim(&hint));
    }
}

pub async fn run(client: &Client, universe_id: u64, json: bool) -> Result<()> {
    let report = collect(client, universe_id).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        render(&report);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn badges_are_ordered_by_awards_and_summed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/universes/42/badges"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[
                   {"id":1,"name":"Rare","enabled":true,"iconImageId":7,
                    "statistics":{"pastDayAwardedCount":2,"awardedCount":40,"winRatePercentage":0.02}},
                   {"id":2,"name":"Welcome","enabled":true,
                    "statistics":{"pastDayAwardedCount":500,"awardedCount":9000,"winRatePercentage":0.9}},
                   {"id":3,"name":"Retired","enabled":false,
                    "statistics":{"pastDayAwardedCount":0,"awardedCount":5,"winRatePercentage":0.0}}],
                   "nextPageCursor":null}"#,
            ))
            .mount(&server)
            .await;

        let report = collect(&Client::with_base_url(&server.uri()), 42)
            .await
            .unwrap();

        assert_eq!(report.badges[0].name, "Welcome");
        assert_eq!(report.badges[2].name, "Retired");
        assert_eq!(report.summary.total, 3);
        assert_eq!(report.summary.enabled, 2);
        assert_eq!(report.summary.awarded_total, 9045);
        assert_eq!(report.summary.awarded_past_day, 502);
    }

    #[tokio::test]
    async fn a_universe_with_no_badges_is_not_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/universes/42/badges"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(r#"{"data":[],"nextPageCursor":null}"#),
            )
            .mount(&server)
            .await;

        let report = collect(&Client::with_base_url(&server.uri()), 42)
            .await
            .unwrap();

        assert_eq!(report.summary.total, 0);
    }
}
