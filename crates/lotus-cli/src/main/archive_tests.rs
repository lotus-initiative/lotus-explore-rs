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

/// A fresh, empty directory under the system temp dir, unique to this test.
fn scratch_dir(test: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lotus-cli-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn write_archive_files_writes_the_three_artifacts_under_their_names() {
    let dir = scratch_dir("write-archive");
    let fetched_at = parse_iso_utc("2025-01-15T12:30:05Z").expect("valid RFC3339");
    let artifacts = lotus_explore_core::export::archive::ArchiveArtifacts {
        archive: vec![0x1f, 0x8b, 0x08],
        manifest: "{\"records\":1}".to_string(),
        checksums: "abc  archive\n".to_string(),
    };

    let (archive, manifest, checksums) =
        write_archive_files(Some(&dir), &archive_filename(fetched_at), &artifacts)
            .expect("files are written");

    assert_eq!(
        archive,
        dir.join("lotus-wikidata-smiles-20250115T123005Z.tar.gz")
    );
    assert_eq!(manifest, dir.join("lotus-wikidata-smiles.manifest.json"));
    assert_eq!(checksums, dir.join("lotus-wikidata-smiles.SHA256SUMS"));
    assert_eq!(
        std::fs::read(&archive).expect("archive"),
        vec![0x1f, 0x8b, 0x08]
    );
    assert_eq!(
        std::fs::read_to_string(&manifest).expect("manifest"),
        "{\"records\":1}"
    );
    assert_eq!(
        std::fs::read_to_string(&checksums).expect("checksums"),
        "abc  archive\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn archive_upload_paths_names_each_upload_after_its_file() {
    let specs = archive_upload_paths(
        std::path::Path::new("out/a.tar.gz"),
        std::path::Path::new("out/m.json"),
        std::path::Path::new("out/SUMS"),
    )
    .expect("three distinct names");
    let uploaded: Vec<(&str, String)> = specs
        .iter()
        .map(|spec| (spec.filename.as_str(), format!("{:?}", spec.source)))
        .collect();
    assert_eq!(
        uploaded,
        vec![
            ("a.tar.gz", "Path(\"out/a.tar.gz\")".to_string()),
            ("m.json", "Path(\"out/m.json\")".to_string()),
            ("SUMS", "Path(\"out/SUMS\")".to_string()),
        ]
    );
}

#[test]
fn a_dry_run_states_the_plan_for_a_first_record_and_for_a_new_version() {
    let (archive, manifest, checksums) = (
        std::path::Path::new("out/a.tar.gz"),
        std::path::Path::new("out/m.json"),
        std::path::Path::new("out/SUMS"),
    );
    assert_eq!(
        dry_run_zenodo(archive, manifest, checksums, None),
        "lotus: Zenodo dry run OK — metadata valid, 3 files ready, plan: publish the \
         initial Zenodo record"
    );
    assert_eq!(
        dry_run_zenodo(
            archive,
            manifest,
            checksums,
            Some(zenodo_rs::DepositionId(42))
        ),
        "lotus: Zenodo dry run OK — metadata valid, 3 files ready, plan: publish a new \
         version from deposition 42"
    );
}

#[test]
fn a_dry_run_reports_an_upload_plan_that_cannot_be_built() {
    // Two files with the same name would collide in the Zenodo bucket.
    let report = dry_run_zenodo(
        std::path::Path::new("one/same"),
        std::path::Path::new("two/same"),
        std::path::Path::new("out/SUMS"),
        None,
    );
    assert_eq!(
        report,
        "lotus: Zenodo dry run failed — upload spec: prepare Zenodo upload specs: \
         duplicate upload filename: same"
    );
}

/// The checksums file must name the archive by the name it is written under, or
/// `sha256sum -c` fails for anyone who downloads the deposit.
#[test]
fn the_checksums_file_names_the_archive_that_was_written() {
    let dir = scratch_dir("checksum-names");
    let fetched_at = parse_iso_utc("2025-01-15T12:30:05Z").expect("valid RFC3339");
    let filename = archive_filename(fetched_at);
    let artifacts = lotus_explore_core::export::archive::write_archive(
        "2025-01-15T12:30:05Z",
        &filename,
        "https://qlever.example/wikidata",
        0,
        &[],
        &[],
        &[],
    )
    .expect("an empty archive serialises");

    let (archive, _, checksums) =
        write_archive_files(Some(&dir), &filename, &artifacts).expect("files are written");

    let written = archive
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .expect("a utf-8 file name");
    let sums = std::fs::read_to_string(&checksums).expect("checksums");
    assert!(
        sums.lines()
            .any(|line| line.ends_with(&format!("  {written}"))),
        "SHA256SUMS does not name {written}:\n{sums}"
    );
}
