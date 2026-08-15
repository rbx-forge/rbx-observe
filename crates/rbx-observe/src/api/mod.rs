//! One HTTP client, one retry loop, five Roblox hosts.
//!
//! Every endpoint reached from here is public: no API key, no cookie, no
//! authentication of any kind. If a change to this module ever needs a
//! credential, the change is wrong — see the boundary in the README.

pub mod badges;
pub mod economy;
pub mod games;
pub mod monetization;
mod pace;
pub mod thumbnails;

use std::time::Duration;

use anyhow::{bail, Context, Result};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;

use pace::Pacer;

/// Named so a Roblox operator reading their logs can tell what this is and
/// where to complain. An anonymous scraper is the thing that gets whole IP
/// ranges blocked for everyone.
const USER_AGENT: &str = concat!(
    "rbx-observe/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/rbx-forge/rbx-observe)"
);

/// The five hosts, kept in one place so tests can point them all at a single
/// mock server. Roblox splits its public read API across subdomains that do
/// not share a version scheme or a pagination style, so the host is part of
/// the identity of an endpoint, not an implementation detail.
#[derive(Clone, Debug)]
pub struct Hosts {
    pub games: String,
    pub badges: String,
    pub apis: String,
    pub economy: String,
    pub thumbnails: String,
}

impl Default for Hosts {
    fn default() -> Self {
        Self {
            games: "https://games.roblox.com".into(),
            badges: "https://badges.roblox.com".into(),
            apis: "https://apis.roblox.com".into(),
            economy: "https://economy.roblox.com".into(),
            thumbnails: "https://thumbnails.roblox.com".into(),
        }
    }
}

pub struct Client {
    http: reqwest::Client,
    hosts: Hosts,
    pacer: Pacer,
    /// First retry delay, doubled per attempt. Zero in tests.
    backoff: Duration,
    attempts: u32,
}

impl Client {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(20))
            .build()
            .context("Failed to build the HTTP client")?;

        Ok(Self {
            http,
            hosts: Hosts::default(),
            pacer: Pacer::new(Duration::from_millis(1000), Duration::from_secs(6)),
            backoff: Duration::from_secs(1),
            attempts: 4,
        })
    }

    /// Points every host at one mock server and removes both the pacing and
    /// the backoff. The standard test-injection idiom in the sibling project,
    /// kept identical here: one way to inject a host, and it cannot leak into
    /// a release build.
    #[cfg(test)]
    pub fn with_base_url(base: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .expect("test client"),
            hosts: Hosts {
                games: base.to_string(),
                badges: base.to_string(),
                apis: base.to_string(),
                economy: base.to_string(),
                thumbnails: base.to_string(),
            },
            pacer: Pacer::disabled(),
            backoff: Duration::ZERO,
            attempts: 2,
        }
    }

    pub fn hosts(&self) -> &Hosts {
        &self.hosts
    }

    /// GET one JSON document.
    ///
    /// Query parameters go through `reqwest`'s builder rather than being
    /// formatted into the URL: pagination cursors are opaque base64 that
    /// contains `+` and `=`, and a cursor pasted into a URL unencoded silently
    /// returns page one forever.
    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<T> {
        let body = self.get_text(url, params).await?;
        serde_json::from_str(&body)
            .with_context(|| format!("Failed to parse the response from {url}"))
    }

    async fn get_text(&self, url: &str, params: &[(&str, &str)]) -> Result<String> {
        let mut last_status: Option<StatusCode> = None;

        for attempt in 0..self.attempts {
            if attempt > 0 && !self.backoff.is_zero() {
                // 1s, 2s, 4s. Doubling rather than a fixed delay because a
                // sliding-window quota needs the window to drain, and a fixed
                // delay just re-enters it at the same rate.
                tokio::time::sleep(self.backoff * 2u32.pow(attempt - 1)).await;
            }
            self.pacer.wait().await;

            let response = self.http.get(url).query(params).send().await;

            let response = match response {
                Ok(response) => response,
                Err(err) => {
                    // A timeout or a connection reset is worth another try; a
                    // URL that will not parse is not, and never becomes one.
                    if err.is_builder() || attempt + 1 == self.attempts {
                        return Err(err).with_context(|| format!("Request to {url} failed"));
                    }
                    continue;
                }
            };

            let status = response.status();
            if status.is_success() {
                return response
                    .text()
                    .await
                    .with_context(|| format!("Failed to read the response body from {url}"));
            }

            if status == StatusCode::TOO_MANY_REQUESTS {
                self.pacer.widen().await;
            }

            last_status = Some(status);
            let retryable = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
            if !retryable {
                break;
            }
        }

        match last_status {
            Some(StatusCode::TOO_MANY_REQUESTS) => bail!(
                "{url}: rate limited by Roblox after {} attempts. The quota is per IP and \
                 per host; wait a minute and run it again.",
                self.attempts
            ),
            Some(status) => bail!("{url}: {status}"),
            None => bail!("{url}: request failed after {} attempts", self.attempts),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Debug, Deserialize)]
    struct Body {
        ok: bool,
    }

    #[tokio::test]
    async fn a_429_is_retried_and_widens_the_pacing() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/thing"))
            .respond_with(ResponseTemplate::new(429))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/thing"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
            .mount(&server)
            .await;

        let client = Client::with_base_url(&server.uri());
        let body: Body = client
            .get_json(&format!("{}/thing", server.uri()), &[])
            .await
            .unwrap();

        assert!(body.ok);
    }

    #[tokio::test]
    async fn a_404_is_not_retried_and_names_the_url() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missing"))
            .respond_with(ResponseTemplate::new(404))
            .expect(1) // The point: one call, not `attempts` calls.
            .mount(&server)
            .await;

        let client = Client::with_base_url(&server.uri());
        let error = client
            .get_json::<Body>(&format!("{}/missing", server.uri()), &[])
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("404"), "{error}");
        assert!(error.contains("/missing"), "{error}");
    }

    #[tokio::test]
    async fn exhausted_retries_explain_the_rate_limit() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/thing"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&server)
            .await;

        let client = Client::with_base_url(&server.uri());
        let error = client
            .get_json::<Body>(&format!("{}/thing", server.uri()), &[])
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains("rate limited"), "{error}");
    }
}
