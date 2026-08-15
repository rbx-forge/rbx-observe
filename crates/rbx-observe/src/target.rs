//! What the user typed → a universe id.
//!
//! Three numbering spaces meet in this tool and none of them is checkable by
//! looking at the number: universe ids, place ids, and asset ids. A game URL
//! carries a **place** id; every read endpoint here takes a **universe** id.
//! Getting that wrong does not fail loudly — see `api::economy` — so the
//! conversion happens once, here, rather than being assumed anywhere else.

use anyhow::{bail, Context, Result};

use crate::api::Client;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Universe(u64),
    Place(u64),
}

impl Target {
    /// `raw` is a bare number or a roblox.com game URL. `place` forces a bare
    /// number to be read as a place id; a URL is always a place id, so the
    /// flag is redundant there rather than contradictory.
    pub fn parse(raw: &str, place: bool) -> Result<Self> {
        let raw = raw.trim();

        if let Some(place_id) = place_id_from_url(raw) {
            return Ok(Target::Place(place_id));
        }

        if raw.contains('/') || raw.contains("roblox.com") {
            bail!(
                "Could not find a place id in {raw:?}. A game URL looks like \
                 https://www.roblox.com/games/<placeId>/<name>."
            );
        }

        let id: u64 = raw
            .parse()
            .with_context(|| format!("{raw:?} is neither a number nor a roblox.com game URL"))?;

        if id == 0 {
            bail!("0 is not a valid id");
        }

        Ok(if place {
            Target::Place(id)
        } else {
            Target::Universe(id)
        })
    }

    pub async fn resolve(self, client: &Client) -> Result<u64> {
        match self {
            Target::Universe(id) => Ok(id),
            Target::Place(id) => client.universe_of_place(id).await,
        }
    }
}

/// Pulls the place id out of any roblox.com game URL shape:
/// `/games/<id>`, `/games/<id>/Name`, with or without scheme, host, query or
/// fragment.
fn place_id_from_url(raw: &str) -> Option<u64> {
    let without_query = raw.split(['?', '#']).next().unwrap_or(raw);
    let (_, after) = without_query.split_once("/games/")?;
    let segment = after.split('/').next()?;
    segment.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_number_is_a_universe_id_by_default() {
        assert_eq!(
            Target::parse("1111111111111", false).unwrap(),
            Target::Universe(1111111111111)
        );
    }

    #[test]
    fn the_place_flag_switches_the_numbering_space() {
        assert_eq!(Target::parse("1818", true).unwrap(), Target::Place(1818));
    }

    #[test]
    fn urls_are_place_ids_whatever_their_shape() {
        let cases = [
            "https://www.roblox.com/games/2222222222222221/Sandbox Frontier",
            "https://roblox.com/games/2222222222222221",
            "www.roblox.com/games/2222222222222221/Sandbox Frontier?privateServerLinkCode=1",
            "roblox.com/games/2222222222222221/Sandbox Frontier#comments",
        ];
        for case in cases {
            assert_eq!(
                Target::parse(case, false).unwrap(),
                Target::Place(2222222222222221),
                "{case}"
            );
        }
    }

    #[test]
    fn a_url_is_a_place_id_even_without_the_flag_and_with_it() {
        // The flag says "read this number as a place id"; a URL already is
        // one, so passing both is redundant rather than contradictory.
        let url = "https://www.roblox.com/games/1818/X";
        assert_eq!(Target::parse(url, true).unwrap(), Target::Place(1818));
        assert_eq!(Target::parse(url, false).unwrap(), Target::Place(1818));
    }

    #[test]
    fn a_url_with_no_id_says_what_a_game_url_looks_like() {
        let error = Target::parse("https://www.roblox.com/discover", false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("https://www.roblox.com/games/"), "{error}");
    }

    #[test]
    fn nonsense_is_refused() {
        assert!(Target::parse("voxels", false).is_err());
        assert!(Target::parse("0", false).is_err());
        assert!(Target::parse("-5", false).is_err());
    }
}
