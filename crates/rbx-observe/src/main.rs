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
    /// Everything the page tells you: players, visits, votes, icon and banner
    /// asset ids, whether the carousel opens on a video.
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
}

impl Command {
    /// Every subcommand takes the same target pair, so resolution happens once
    /// here rather than being repeated in four command modules.
    fn target(&self) -> Result<Target> {
        let (raw, place) = match self {
            Command::Game { target, place }
            | Command::Storefront { target, place }
            | Command::Badges { target, place }
            | Command::Media { target, place } => (target, *place),
        };
        Target::parse(raw, place)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = Client::new()?;
    let universe_id = cli.command.target()?.resolve(&client).await?;

    match cli.command {
        Command::Game { .. } => commands::game::run(&client, universe_id, cli.json).await,
        Command::Storefront { .. } => {
            commands::storefront::run(&client, universe_id, cli.json).await
        }
        Command::Badges { .. } => commands::badges::run(&client, universe_id, cli.json).await,
        Command::Media { .. } => commands::media::run(&client, universe_id, cli.json).await,
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
        assert_eq!(cli.command.target().unwrap(), Target::Place(123));
    }
}
