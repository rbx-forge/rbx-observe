//! The binary's own contract: what `--help` promises, which exit code a
//! failure carries, and whether a bad argument is caught before anything is
//! sent to Roblox.
//!
//! Every case here runs **offline**. Nothing in this file may reach the
//! network: a test suite that calls a third party is a test suite that fails
//! when that third party has a bad day, and this one would be doing it from
//! everybody's CI at once.

use assert_cmd::Command;
use predicates::str::contains;

fn rbx_observe() -> Command {
    Command::cargo_bin("rbx-observe").expect("the binary is built by `cargo test`")
}

#[test]
fn help_lists_every_command() {
    let assert = rbx_observe().arg("--help").assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).to_string();

    for command in [
        "charts",
        "game",
        "storefront",
        "badges",
        "media",
        "places",
        "group",
        "asset",
    ] {
        assert!(
            out.contains(command),
            "`{command}` missing from --help:\n{out}"
        );
    }
}

#[test]
fn version_prints_the_manifest_version() {
    rbx_observe()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn an_unsupported_render_size_is_refused_before_any_request() {
    // clap exits 2 on a usage error, which is what separates "you typed it
    // wrong" from "Roblox said no" (exit 1). Offline by construction: the
    // value parser rejects it before the client is ever used.
    rbx_observe()
        .args(["asset", "1", "--size", "421x421"])
        .assert()
        .code(2)
        .stderr(contains("421x421"));
}

#[test]
fn asset_needs_at_least_one_id() {
    rbx_observe().arg("asset").assert().code(2);
}

#[test]
fn a_target_that_is_neither_a_number_nor_a_url_says_so() {
    // Exit 1, not 2: the arguments parsed fine, the value in them did not.
    rbx_observe()
        .args(["game", "voxels"])
        .assert()
        .code(1)
        .stderr(contains("neither a number nor a roblox.com game URL"));
}

#[test]
fn a_roblox_url_with_no_place_id_shows_the_shape_it_expected() {
    rbx_observe()
        .args(["game", "https://www.roblox.com/discover"])
        .assert()
        .code(1)
        .stderr(contains("https://www.roblox.com/games/"));
}

#[test]
fn zero_is_not_an_id() {
    rbx_observe()
        .args(["storefront", "0"])
        .assert()
        .code(1)
        .stderr(contains("not a valid id"));
}

#[test]
fn an_unknown_command_is_a_usage_error() {
    rbx_observe().arg("observe-everything").assert().code(2);
}

#[test]
fn json_is_accepted_on_either_side_of_the_subcommand() {
    // Both orders have to parse. Only the argument handling is under test, so
    // a target that fails before any request keeps this offline.
    for args in [
        vec!["--json", "game", "voxels"],
        vec!["game", "voxels", "--json"],
    ] {
        rbx_observe()
            .args(&args)
            .assert()
            .code(1)
            .stderr(contains("neither a number"));
    }
}
