//! `rbx-observe` reads the **public** storefront of a Roblox experience.
//!
//! # The boundary
//!
//! Game-level, aggregate, public data only. Never person-level: no player
//! lists, no account tracking, no per-user history. That line separates market
//! research from surveillance, and it is a design constraint rather than a
//! policy: every endpoint added here has to be justifiable against it.
//!
//! # Layout
//!
//! This file is the clap surface and dispatch, nothing else. `api/` holds one
//! module per Roblox host with one method per endpoint, `commands/` holds the
//! logic, `target.rs` resolves whatever the user typed into a universe id.

mod api;
mod commands;
mod render;
mod target;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::api::Client;
use crate::target::Target;

#[derive(Parser)]
#[command(name = "rbx-observe", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Print JSON instead of the human rendering.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Everything the page tells you: description, players, visits, votes,
    /// maturity label, every place in the universe, icon and banner asset ids.
    Game {
        /// Universe id, place id, or a roblox.com game URL.
        target: String,
        /// Read the number as a place id rather than a universe id.
        ///
        /// A game URL carries a place id, and the two are separate numbering
        /// spaces — passing one where the other belongs is the classic way to
        /// get plausible data about something else entirely.
        #[arg(long)]
        place: bool,
    },

    /// What the experience sells: game passes and developer products, with
    /// prices and icon asset ids.
    Storefront {
        /// Universe id, place id, or a roblox.com game URL.
        target: String,
        /// Read the number as a place id rather than a universe id.
        #[arg(long)]
        place: bool,
    },

    /// Badges the experience publishes, with award counts and win rates.
    Badges {
        /// Universe id, place id, or a roblox.com game URL.
        target: String,
        /// Read the number as a place id rather than a universe id.
        #[arg(long)]
        place: bool,
    },

    /// The game page carousel: screenshot asset ids and preview video.
    Media {
        /// Universe id, place id, or a roblox.com game URL.
        target: String,
        /// Read the number as a place id rather than a universe id.
        #[arg(long)]
        place: bool,
    },

    /// Everything about one experience, as one document worth keeping.
    ///
    /// The summary prints; `--json` carries the whole thing, which is what to
    /// redirect into a dated file and compare later.
    Snapshot {
        /// Universe id, place id, or a roblox.com game URL.
        target: String,
        /// Read the number as a place id rather than a universe id.
        #[arg(long)]
        place: bool,

        /// Also probe every place: published state and live servers. Two
        /// requests per place, none of which batch.
        #[arg(long)]
        places: bool,
    },

    /// Every place in the universe: whether each looks published, and what is
    /// running on it right now.
    ///
    /// Separate from `game` because it costs two extra requests per place.
    Places {
        /// Universe id, place id, or a roblox.com game URL.
        target: String,
        /// Read the number as a place id rather than a universe id.
        #[arg(long)]
        place: bool,
    },

    /// What Roblox is pushing right now: its own rankings.
    ///
    /// The discovery command — every other one needs a universe id, this one
    /// hands them out.
    Charts {
        /// One sort only, by id (`top-playing-now`, `top-earning`,
        /// `up-and-coming`, `trending-in-<category>`, …).
        #[arg(long)]
        sort: Option<String>,

        /// One category only, matching the suffix of a `trending-in-…` sort
        /// (`obby-and-platformer`, `survival`, …). Roblox's real taxonomy.
        #[arg(long)]
        category: Option<String>,

        /// Rows per sort in the human rendering. `--json` is never truncated.
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },

    /// A studio and the games it publishes.
    ///
    /// There is no user-keyed equivalent: a catalog keyed to an individual
    /// account is a person's output rather than a studio's.
    Group {
        /// Group id, from `roblox.com/communities/<groupId>/...`.
        group_id: u64,

        /// Also list the games that are not in the group's public listing.
        ///
        /// Roblox returns a group's staging copies, test places and
        /// unreleased projects to anonymous callers. They are always counted
        /// in the summary; this prints their names.
        #[arg(long)]
        all: bool,
    },

    /// Turn asset ids into the URLs that render them.
    ///
    /// Every other command prints asset ids; this resolves any of them, in one
    /// batched call. Only a completed render is reported as a URL: Roblox
    /// answers a bad id with a placeholder image rather than an error.
    Asset {
        /// One or more asset ids.
        #[arg(required = true, num_args = 1..)]
        asset_ids: Vec<u64>,

        /// Render size. Roblox rejects anything outside this list.
        #[arg(long, default_value = "420x420", value_parser = api::thumbnails::SIZES)]
        size: String,
    },
}

impl Command {
    /// The four universe-keyed commands take the same target pair, so parsing
    /// happens once here rather than in four command modules. `group` and
    /// `asset` are keyed by something else and have no target.
    fn target(&self) -> Result<Option<Target>> {
        let (raw, place) = match self {
            Command::Game { target, place }
            | Command::Storefront { target, place }
            | Command::Badges { target, place }
            | Command::Media { target, place }
            | Command::Places { target, place }
            | Command::Snapshot { target, place, .. } => (target, *place),
            Command::Charts { .. } | Command::Group { .. } | Command::Asset { .. } => {
                return Ok(None)
            }
        };
        Target::parse(raw, place).map(Some)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = Client::new()?;

    // Resolved before dispatch so that a place id costs its one extra lookup
    // in exactly one place, whichever command asked for it.
    let universe_id = match cli.command.target()? {
        Some(target) => target.resolve(&client).await?,
        None => 0,
    };

    match cli.command {
        Command::Game { .. } => commands::game::run(&client, universe_id, cli.json).await,
        Command::Storefront { .. } => {
            commands::storefront::run(&client, universe_id, cli.json).await
        }
        Command::Badges { .. } => commands::badges::run(&client, universe_id, cli.json).await,
        Command::Media { .. } => commands::media::run(&client, universe_id, cli.json).await,
        Command::Places { .. } => commands::places::run(&client, universe_id, cli.json).await,
        Command::Snapshot { places, .. } => {
            commands::snapshot::run(&client, universe_id, places, cli.json).await
        }
        Command::Charts {
            sort,
            category,
            limit,
        } => {
            commands::charts::run(
                &client,
                sort.as_deref(),
                category.as_deref(),
                limit,
                cli.json,
            )
            .await
        }
        Command::Group { group_id, all } => {
            commands::group::run(&client, group_id, all, cli.json).await
        }
        Command::Asset { asset_ids, size } => {
            commands::asset::run(&client, &asset_ids, &size, cli.json).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        // clap's own consistency check: conflicting flags, duplicate names,
        // impossible arg combinations. Catches at test time what would
        // otherwise panic on the user's first invocation.
        Cli::command().debug_assert();
    }

    #[test]
    fn json_is_accepted_after_the_subcommand() {
        // `global = true` is what allows this. Without it `rbx-observe game 1
        // --json` fails, which is the order everyone types.
        let cli = Cli::try_parse_from(["rbx-observe", "game", "123", "--json"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn place_flag_reaches_the_target() {
        let cli = Cli::try_parse_from(["rbx-observe", "game", "123", "--place"]).unwrap();
        assert_eq!(cli.command.target().unwrap(), Some(Target::Place(123)));
    }

    #[test]
    fn group_and_asset_have_no_universe_target_to_resolve() {
        let group = Cli::try_parse_from(["rbx-observe", "group", "33333333333"]).unwrap();
        assert_eq!(group.command.target().unwrap(), None);

        let asset = Cli::try_parse_from(["rbx-observe", "asset", "1", "2"]).unwrap();
        assert_eq!(asset.command.target().unwrap(), None);
    }

    #[test]
    fn an_unsupported_render_size_is_refused_before_any_request() {
        assert!(Cli::try_parse_from(["rbx-observe", "asset", "1", "--size", "421x421"]).is_err());
        assert!(Cli::try_parse_from(["rbx-observe", "asset", "1", "--size", "512x512"]).is_ok());
    }
}
