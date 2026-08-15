//! `rbx-observe` reads the **public** storefront of a Roblox experience.
//!
//! # The boundary
//!
//! Game-level, aggregate, public data only. Never person-level: no player
//! lists, no account tracking, no per-user history. That line is what
//! separates market research from surveillance, and it is a design constraint
//! here rather than a policy that could be relaxed later — every endpoint this
//! tool learns to call has to be justifiable against it.
//!
//! # Status
//!
//! Skeleton. The CLI surface below parses and nothing behind it is
//! implemented; each subcommand says so rather than pretending.

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rbx-observe", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Everything a universe sells: passes, developer products, prices.
    Storefront {
        /// Universe id to read.
        universe_id: u64,
    },
    /// Badges a universe publishes, with their award counts.
    Badges {
        /// Universe id to read.
        universe_id: u64,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Storefront { .. } => bail!("storefront: not implemented yet"),
        Command::Badges { .. } => bail!("badges: not implemented yet"),
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
}
