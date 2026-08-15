//! `rbx-observe asset` — asset ids in, image URLs out.
//!
//! The companion to every other command: they print asset ids, this turns any
//! of them into something you can open or download, in one batched call.
//!
//! It resolves the **rendered** image. The original uploaded file lives behind
//! `assetdelivery.roblox.com`, which answers 401 without a session — that is
//! `rbx download`'s job in the sibling project, and it is why this tool stops
//! at the render.

use anyhow::Result;
use serde::Serialize;

use crate::api::thumbnails::Thumbnail;
use crate::api::Client;
use crate::render::{dim, heading};

#[derive(Debug, Serialize)]
pub struct Assets {
    pub size: String,
    pub resolved: Vec<Resolved>,
}

#[derive(Debug, Serialize)]
pub struct Resolved {
    pub asset_id: u64,
    pub url: Option<String>,
    /// `Completed`, `Pending`, `Blocked`, `Error`, or `Unanswered` when Roblox
    /// returned nothing at all for an id we asked about.
    pub state: String,
}

pub async fn collect(client: &Client, ids: &[u64], size: &str) -> Result<Assets> {
    let thumbnails = client.asset_thumbnails(ids, size).await?;

    // Keyed back to the requested ids rather than returned in response order:
    // Roblox omits ids it has nothing for, and a silently shorter list would
    // shift every row after the gap.
    let resolved = ids
        .iter()
        .map(|id| {
            match thumbnails
                .iter()
                .find(|thumbnail: &&Thumbnail| thumbnail.target_id == *id)
            {
                // Only a `Completed` render is a real URL. An id that does
                // not exist, or names something with no image, comes back as
                // `Error` **with an imageUrl** — the grey "no thumbnail"
                // placeholder, byte-identical for every bad id. Passing that
                // through would hand back a working link for a wrong id,
                // which is worse than saying nothing.
                Some(thumbnail) if thumbnail.state == "Completed" => Resolved {
                    asset_id: *id,
                    url: thumbnail.image_url.clone().filter(|url| !url.is_empty()),
                    state: thumbnail.state.clone(),
                },
                Some(thumbnail) => Resolved {
                    asset_id: *id,
                    url: None,
                    state: thumbnail.state.clone(),
                },
                None => Resolved {
                    asset_id: *id,
                    url: None,
                    state: "Unanswered".to_string(),
                },
            }
        })
        .collect();

    Ok(Assets {
        size: size.to_string(),
        resolved,
    })
}

pub fn render(assets: &Assets) {
    println!("{}", heading(&format!("Assets at {}", assets.size)));
    for entry in &assets.resolved {
        match &entry.url {
            Some(url) => {
                println!("  {}", entry.asset_id);
                println!("    {url}");
            }
            None => println!("  {}  {}", entry.asset_id, dim(&entry.state)),
        }
    }
}

pub async fn run(client: &Client, ids: &[u64], size: &str, json: bool) -> Result<()> {
    let assets = collect(client, ids, size).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&assets)?);
    } else {
        render(&assets);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn every_requested_id_gets_a_row_even_the_ones_roblox_ignores() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/assets"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"targetId":2,"state":"Completed","imageUrl":"https://tr.rbxcdn.com/x"}]}"#,
            ))
            .mount(&server)
            .await;

        let assets = collect(&Client::with_base_url(&server.uri()), &[1, 2], "420x420")
            .await
            .unwrap();

        assert_eq!(assets.resolved.len(), 2);
        assert_eq!(assets.resolved[0].asset_id, 1);
        assert_eq!(assets.resolved[0].state, "Unanswered");
        assert_eq!(
            assets.resolved[1].url.as_deref(),
            Some("https://tr.rbxcdn.com/x")
        );
    }

    #[tokio::test]
    async fn a_bad_id_reports_its_state_instead_of_the_placeholder_url() {
        let server = MockServer::start().await;
        // Recorded verbatim from the live endpoint for a nonexistent id: an
        // `Error` state carrying the grey placeholder image, which is handed
        // back identically for every invalid id.
        Mock::given(method("GET"))
            .and(path("/v1/assets"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[{"targetId":999999999999999,"state":"Error",
                   "imageUrl":"https://t2.rbxcdn.com/180DAY-a53354b1f60a5dedc00d0600d1491075"}]}"#,
            ))
            .mount(&server)
            .await;

        let assets = collect(
            &Client::with_base_url(&server.uri()),
            &[999999999999999],
            "420x420",
        )
        .await
        .unwrap();

        assert_eq!(assets.resolved[0].state, "Error");
        assert_eq!(assets.resolved[0].url, None);
    }
}
