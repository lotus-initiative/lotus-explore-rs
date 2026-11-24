// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::ExportFormat;

#[test]
fn parse_download_format_supports_documented_aliases() {
    assert_eq!(ExportFormat::parse("csv"), Some(super::ExportFormat::Csv));
    assert_eq!(ExportFormat::parse("json"), Some(ExportFormat::Json));
    assert_eq!(ExportFormat::parse("ndjson"), Some(ExportFormat::Json));
    assert_eq!(ExportFormat::parse("ttl"), Some(ExportFormat::Rdf));
    assert_eq!(ExportFormat::parse(" JSON "), Some(ExportFormat::Json));
    assert_eq!(ExportFormat::parse("TTL"), Some(ExportFormat::Rdf));
    // Still accepted so export links handed out before the rename keep resolving.
    assert_eq!(ExportFormat::parse("rdf"), Some(ExportFormat::Rdf));
    assert_eq!(ExportFormat::parse("turtle"), None);
}

#[test]
fn gz_or_plain_url_prefers_non_empty_gz() {
    assert_eq!(
        super::gz_or_plain_url(Some("https://x/file.csv.gz"), "https://x/file.csv"),
        "https://x/file.csv.gz"
    );
}

#[test]
fn gz_or_plain_url_falls_back_to_plain_when_gz_empty() {
    assert_eq!(
        super::gz_or_plain_url(Some(""), "https://x/file.csv"),
        "https://x/file.csv"
    );
    assert_eq!(
        super::gz_or_plain_url(Some("   "), "https://x/file.csv"),
        "https://x/file.csv"
    );
    assert_eq!(
        super::gz_or_plain_url(None, "https://x/file.csv"),
        "https://x/file.csv"
    );
    assert_eq!(super::gz_or_plain_url(None, ""), "");
}
