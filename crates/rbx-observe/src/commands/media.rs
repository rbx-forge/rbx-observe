//! `rbx-observe media` — the game page carousel.
//!
//! This is the command that exists purely for asset ids: `imageId` here is a
//! permanent asset id, not a CDN hash, so every screenshot on the page can be
//! rendered, compared across runs, or diffed against your own.

use anyhow::Result;
use serde::Serialize;

use crate::api::games::MediaEntry;
use crate::api::Client;
use crate::render::{asset_url, dim, heading};

#[derive(Debug, Serialize)]
pub struct Media {
    pub universe_id: u64,
    pub entries: Vec<MediaEntry>,
    pub image_asset_ids: Vec<u64>,
    pub has_preview_video: bool,
}

pub async fn collect(client: &Client, universe_id: u64) -> Result<Media> {
    let entries = client.media(universe_id).await?;

    Ok(Media {
        universe_id,
        image_asset_ids: entries
            .iter()
            .filter(|entry| !entry.is_video())
            .filter_map(|entry| entry.image_id)
            .collect(),
        has_preview_video: entries.iter().any(MediaEntry::is_video),
        entries,
    })
}

pub fn render(media: &Media) {
    println!("{}", heading("Carousel"));
    if media.entries.is_empty() {
        println!("  {}", dim("empty"));
    }

    for (index, entry) in media.entries.iter().enumerate() {
        let position = index + 1;
        if entry.is_video() {
            let title = entry.video_title.as_deref().unwrap_or("preview video");
            let reference = entry
                .video_id
                .as_deref()
                .or(entry.video_hash.as_deref())
                .unwrap_or("-");
            println!("  {position}. video   {title}");
            println!("     {}", dim(&format!("ref {reference}")));
            continue;
        }

        match entry.image_id {
            Some(id) => {
                println!("  {position}. image   asset {id}");
                println!("     {}", dim(&asset_url(id)));
            }
            None => println!("  {position}. image   {}", dim("no asset id")),
        }
        if let Some(alt) = entry.alt_text.as_deref().filter(|text| !text.is_empty()) {
            println!("     {}", dim(&format!("alt: {alt}")));
        }
    }

    println!();
    println!(
        "  {} image(s), {}",
        media.image_asset_ids.len(),
        if media.has_preview_video {
            "opens on a video"
        } else {
            "no preview video"
        }
    );
}

pub async fn run(client: &Client, universe_id: u64, json: bool) -> Result<()> {
    let media = collect(client, universe_id).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&media)?);
    } else {
        render(&media);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn video_entries_are_flagged_and_kept_out_of_the_asset_id_list() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/games/42/media"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"data":[
                   {"assetTypeId":86,"assetType":"GamePreviewVideo","videoHash":"h","imageId":1},
                   {"assetTypeId":1,"assetType":"Image","imageId":4444444444444443,"altText":"lobby"},
                   {"assetTypeId":1,"assetType":"Image","imageId":4444444444444444}]}"#,
            ))
            .mount(&server)
            .await;

        let media = collect(&Client::with_base_url(&server.uri()), 42)
            .await
            .unwrap();

        assert!(media.has_preview_video);
        // The video entry carries an imageId too — its poster frame. Counting
        // it as a screenshot would inflate every carousel with a video by one.
        assert_eq!(media.image_asset_ids, vec![4444444444444443, 4444444444444444]);
    }
}
