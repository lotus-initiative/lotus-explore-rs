// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `config`, in their own file.

#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::indexing_slicing)]

use super::*;

// clap's `#[arg(env = "...")]` guarantees flag > env > default priority;
// these tests verify flag resolution and defaults directly. Env-only
// overrides are untestable here: the workspace enforces
// `#![forbid(unsafe_code)]` and `std::env::set_var` / `remove_var` are unsafe
// in Rust 1.97+. The `from_provider` tests in tests.rs cover the env-value
// parsing layer with mock closures.

#[test]
fn cli_port_flag_overrides_default() {
    let cli = Cli::try_parse_from(["lotus-explore-rs", "--port", "1234"]).unwrap();
    assert_eq!(cli.get("PORT"), Some("1234".to_string()));
}

#[test]
fn cli_port_default_when_no_flag() {
    // Assumes PORT is not set in the test environment (typical for CI).
    let cli = Cli::try_parse_from(["lotus-explore-rs"]).unwrap();
    assert_eq!(cli.get("PORT"), Some("8787".to_string()));
}

#[test]
fn cli_host_flag_overrides_default() {
    let cli = Cli::try_parse_from(["lotus-explore-rs", "--host", "0.0.0.0"]).unwrap();
    assert_eq!(cli.get("HOST"), Some("0.0.0.0".to_string()));
}

/// One shape, three rejections: non-numeric, negative, and past `u16::MAX`.
/// Each must name `PORT` in the message, so that is what the table asserts.
#[test]
fn from_provider_rejects_a_port_it_cannot_parse() {
    for bad in ["not-a-port", "-1", "70000"] {
        let result = AppConfig::from_provider(|name| (name == "PORT").then(|| bad.to_string()));
        let err = result.expect_err("an unparseable port should error");
        assert!(err.contains("PORT"), "{bad:?} should be named in the error");
    }
}

#[test]
fn flag_port_through_full_flow() {
    let cli = Cli::try_parse_from(["lotus-explore-rs", "--port", "1234"]).unwrap();
    let cfg = AppConfig::from_provider(|name| cli.get(name)).expect("flag port 1234 should parse");
    assert_eq!(cfg.port, 1234);
}

#[test]
fn default_port_through_full_flow() {
    // Assumes PORT is not set in the test environment.
    let cli = Cli::try_parse_from(["lotus-explore-rs"]).unwrap();
    let cfg = AppConfig::from_provider(|name| cli.get(name)).expect("default port should parse");
    assert_eq!(cfg.port, 8787);
}
