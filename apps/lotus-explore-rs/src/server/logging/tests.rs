// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit tests for server log level parsing.

use super::parse_log_level;
use log::LevelFilter;

#[test]
fn parses_plain_directives_case_insensitively() {
    assert_eq!(parse_log_level(None), LevelFilter::Info);
    assert_eq!(parse_log_level(Some("")), LevelFilter::Info);
    assert_eq!(parse_log_level(Some("garbage")), LevelFilter::Info);
    assert_eq!(parse_log_level(Some("off")), LevelFilter::Off);
    assert_eq!(parse_log_level(Some("ERROR")), LevelFilter::Error);
    assert_eq!(parse_log_level(Some("warn")), LevelFilter::Warn);
    assert_eq!(parse_log_level(Some("info")), LevelFilter::Info);
    assert_eq!(parse_log_level(Some("Debug")), LevelFilter::Debug);
    assert_eq!(parse_log_level(Some("trace")), LevelFilter::Trace);
}

#[test]
fn parses_target_scoped_comma_directives() {
    assert_eq!(
        parse_log_level(Some("lotus_explore_rs=debug,tower_http=warn")),
        LevelFilter::Debug
    );
    assert_eq!(
        parse_log_level(Some("warn,my_crate=off")),
        LevelFilter::Warn
    );
}
