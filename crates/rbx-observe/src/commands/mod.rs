//! One module per subcommand.
//!
//! Each is the same three pieces: `collect` does the reading and returns data,
//! `render` prints it for a human, `run` wires them together and picks between
//! the human and the JSON rendering. Keeping `collect` free of printing is
//! what lets the wiremock tests assert on data instead of scraping stdout.

pub mod asset;
pub mod badges;
pub mod charts;
pub mod game;
pub mod group;
pub mod media;
pub mod places;
pub mod snapshot;
pub mod storefront;

/// Test support for the commands that print an experience's creator.
#[cfg(test)]
pub(crate) mod boundary {
    use wiremock::MockServer;

    /// Asserts that a run printed the creator without following it anywhere.
    ///
    /// `Client::with_base_url` points every host at the mock, so its request
    /// log is the whole traffic of the run. No request may carry the
    /// creator's id, in its URL or in its body, and none may be keyed to a
    /// person: a user, a group's owner, a group's roster.
    pub(crate) async fn assert_creator_not_followed(server: &MockServer, creator_id: u64) {
        let id = creator_id.to_string();
        let requests = server
            .received_requests()
            .await
            .expect("wiremock records requests by default");
        assert!(!requests.is_empty(), "the run made no request at all");

        for request in &requests {
            let url = request.url.as_str();
            let body = String::from_utf8_lossy(&request.body);
            assert!(!url.contains(&id), "request about the creator: {url}");
            assert!(
                !body.contains(&id),
                "the creator's id in the body of {url}: {body}"
            );
            for keyed in ["/users", "/owner", "/roles"] {
                assert!(!url.contains(keyed), "request keyed to a person: {url}");
            }
        }
    }
}
