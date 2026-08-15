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
