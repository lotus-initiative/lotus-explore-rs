// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Tests for the `archive` subcommand's helper functions.
#![allow(clippy::expect_used)]

use super::*;

const VALID_SHA: &str = "0123456789abcdef0123456789abcdef01234567";

#[test]
fn validate_source_code_url_accepts_https_with_commit_sha() {
    let url = format!("https://github.com/lotusnprod/lotus-explore-rs/commit/{VALID_SHA}");
    assert_eq!(validate_source_code_url(&url).expect("url is valid"), url);
}

#[test]
fn validate_source_code_url_rejects_http() {
    let url = format!("http://github.com/lotusnprod/lotus-explore-rs/commit/{VALID_SHA}");
    assert!(validate_source_code_url(&url).is_err());
}

#[test]
fn validate_source_code_url_rejects_missing_commit_suffix() {
    assert!(validate_source_code_url("https://github.com/lotusnprod/lotus-explore-rs").is_err());
}

#[test]
fn validate_source_code_url_rejects_short_sha() {
    let url = "https://github.com/lotusnprod/lotus-explore-rs/commit/abc123".to_string();
    assert!(validate_source_code_url(&url).is_err());
}

#[test]
fn validate_source_code_url_rejects_non_hex_sha() {
    let url = format!(
        "https://github.com/lotusnprod/lotus-explore-rs/commit/{}",
        "z".repeat(40)
    );
    assert!(validate_source_code_url(&url).is_err());
}

#[test]
fn parse_iso_utc_parses_valid_rfc3339() {
    use chrono::Datelike;
    let dt = parse_iso_utc("2025-01-15T12:30:00Z").expect("valid RFC3339");
    assert_eq!(dt.year(), 2025);
    assert_eq!(dt.month(), 1);
    assert_eq!(dt.day(), 15);
}

#[test]
fn parse_deposition_id_some_when_present() {
    let result = parse_deposition_id("42").expect("42 is numeric");
    assert!(result.is_some());
    assert_eq!(result.expect("42 parsed").0, 42);
}

#[test]
fn parse_deposition_id_rejects_non_numeric() {
    let result = parse_deposition_id("not-a-number");
    assert!(result.is_err());
}

#[test]
fn parse_deposition_id_rejects_zero() {
    let result = parse_deposition_id("0");
    assert!(result.is_err());
}

#[test]
fn zenodo_metadata_builds_with_valid_inputs() {
    let url = format!("https://github.com/lotusnprod/lotus-explore-rs/commit/{VALID_SHA}");
    let metadata = zenodo_metadata(
        "Test Title",
        "Test Creator",
        chrono::NaiveDate::from_ymd_opt(2025, 1, 15).expect("valid date"),
        &url,
    )
    .expect("valid metadata");
    assert_eq!(metadata.title, "Test Title".to_string());
    assert_eq!(metadata.creators.len(), 1);
    assert_eq!(metadata.upload_type, zenodo_rs::UploadType::Dataset);
    assert_eq!(metadata.access_right, zenodo_rs::AccessRight::Open);
    assert_eq!(metadata.license, Some("cc-by-4.0".to_string()));
    assert_eq!(metadata.keywords.len(), 3);
    assert_eq!(metadata.related_identifiers.len(), 1);
}
