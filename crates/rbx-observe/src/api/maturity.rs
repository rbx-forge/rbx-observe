//! `apis.roblox.com/experience-guidelines-api` — the maturity label the game
//! page shows above the description ("Minimal", "Mild", "Moderate",
//! "Restricted") and the content descriptors behind it.
//!
//! The only endpoint in this tool that is a POST: the universe id goes in a
//! JSON body rather than a query string. It is still a read, and still
//! anonymous.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::Client;

/// Two independent axes that Roblox ships in one object and the page shows as
/// one string ("Maturity: Minimal • Ages 16+"):
///
/// - **maturity** (`display_name`) describes the *content*: Minimal, Mild,
///   Moderate, Restricted.
/// - **age** (`minimum_age`) is the *gate*: who is allowed in at all.
///
/// They do not track each other. Two experiences can both be `Minimal` while
/// one is open to everyone and the other is 16+, which is exactly why this
/// tool prints them on separate lines instead of merging them.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgeRecommendation {
    /// `Minimal`, `Mild`, `Moderate`, `Restricted`, or absent while Roblox is
    /// still computing one for a young experience.
    #[serde(default)]
    pub display_name: Option<String>,
    /// `0` means no age gate, not "unrated".
    #[serde(default)]
    pub minimum_age: Option<u32>,
    /// Roblox's own rendering of the gate: `16+`, or an empty string when
    /// there is none. Kept because it is the label the page shows.
    #[serde(default)]
    pub minimum_age_display: Option<String>,
    #[serde(default)]
    pub content_maturity: Option<String>,
}

/// One content descriptor, e.g. "Violence (Repeated/Mild)". The nested
/// dimension values (frequency, intensity) are already folded into
/// `display_name` by Roblox, so they are not read separately.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Maturity {
    pub recommendation: AgeRecommendation,
    pub descriptors: Vec<Descriptor>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Response {
    #[serde(default)]
    age_recommendation_details: Option<Details>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Details {
    #[serde(default)]
    summary: Option<Summary>,
    #[serde(default = "Vec::new")]
    descriptor_usages: Vec<DescriptorUsage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Summary {
    #[serde(default)]
    age_recommendation: Option<AgeRecommendation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DescriptorUsage {
    #[serde(default)]
    descriptor: Option<Descriptor>,
    /// Roblox lists descriptors it checked and did **not** find, with
    /// `contains: false`. Printing those would say a game contains things it
    /// does not.
    #[serde(default)]
    contains: bool,
}

impl Client {
    pub async fn maturity(&self, universe_id: u64) -> Result<Maturity> {
        let url = format!(
            "{}/experience-guidelines-api/experience-guidelines/get-age-recommendation",
            self.hosts().apis
        );
        let body = serde_json::json!({ "universeId": universe_id });
        let response: Response = self.post_json(&url, &body).await?;

        let Some(details) = response.age_recommendation_details else {
            return Ok(Maturity::default());
        };

        Ok(Maturity {
            recommendation: details
                .summary
                .and_then(|summary| summary.age_recommendation)
                .unwrap_or_default(),
            descriptors: details
                .descriptor_usages
                .into_iter()
                .filter(|usage| usage.contains)
                .filter_map(|usage| usage.descriptor)
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const ENDPOINT: &str =
        "/experience-guidelines-api/experience-guidelines/get-age-recommendation";

    #[tokio::test]
    async fn the_universe_id_travels_in_the_body_and_descriptors_are_filtered() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(ENDPOINT))
            .and(body_json(
                serde_json::json!({ "universeId": 1111111111111u64 }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"ageRecommendationDetails":{
                     "summary":{"ageRecommendation":{"displayName":"Mild","minimumAge":0,
                                "contentMaturity":"mild"}},
                     "descriptorUsages":[
                       {"descriptor":{"name":"violence","displayName":"Violence (Repeated/Mild)"},
                        "contains":true},
                       {"descriptor":{"name":"blood","displayName":"Blood"},"contains":false}]}}"#,
            ))
            .expect(1)
            .mount(&server)
            .await;

        let maturity = Client::with_base_url(&server.uri())
            .maturity(1111111111111)
            .await
            .unwrap();

        assert_eq!(
            maturity.recommendation.display_name.as_deref(),
            Some("Mild")
        );
        assert_eq!(maturity.recommendation.minimum_age, Some(0));
        // `blood` was checked and not found; reporting it would say the game
        // contains something it does not.
        assert_eq!(maturity.descriptors.len(), 1);
        assert_eq!(
            maturity.descriptors[0].display_name.as_deref(),
            Some("Violence (Repeated/Mild)")
        );
    }

    #[tokio::test]
    async fn the_age_gate_is_read_separately_from_the_maturity_label() {
        let server = MockServer::start().await;
        // Recorded live from an experience gated at 16+: the maturity label is
        // `Minimal`, the same as an all-ages game. Only `minimumAge` differs,
        // which is why the two are not one field.
        Mock::given(method("POST"))
            .and(path(ENDPOINT))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"ageRecommendationDetails":{"summary":{"ageRecommendation":{
                     "displayName":"Minimal","minimumAge":16,
                     "displayNameWithHeaderShort":"Maturity: Minimal • Ages 16+",
                     "minimumAgeDisplay":"16+","contentMaturity":"minimal"}}}}"#,
            ))
            .mount(&server)
            .await;

        let maturity = Client::with_base_url(&server.uri())
            .maturity(1111111111113)
            .await
            .unwrap();

        assert_eq!(
            maturity.recommendation.display_name.as_deref(),
            Some("Minimal")
        );
        assert_eq!(maturity.recommendation.minimum_age, Some(16));
        assert_eq!(
            maturity.recommendation.minimum_age_display.as_deref(),
            Some("16+")
        );
    }

    #[tokio::test]
    async fn an_unrated_experience_yields_an_empty_maturity_rather_than_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(ENDPOINT))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{}"#))
            .mount(&server)
            .await;

        let maturity = Client::with_base_url(&server.uri())
            .maturity(1)
            .await
            .unwrap();

        assert!(maturity.recommendation.display_name.is_none());
        assert!(maturity.descriptors.is_empty());
    }
}
