// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::DownloadFormat;

#[test]
fn parse_download_format_supports_documented_aliases() {
    assert_eq!(
        DownloadFormat::parse("csv"),
        Some(super::DownloadFormat::Csv)
    );
    assert_eq!(DownloadFormat::parse("json"), Some(DownloadFormat::Json));
    assert_eq!(DownloadFormat::parse("ndjson"), Some(DownloadFormat::Json));
    assert_eq!(DownloadFormat::parse("ttl"), Some(DownloadFormat::Rdf));
    assert_eq!(DownloadFormat::parse(" JSON "), Some(DownloadFormat::Json));
    assert_eq!(DownloadFormat::parse("TTL"), Some(DownloadFormat::Rdf));
    // Still accepted so export links handed out before the rename keep resolving.
    assert_eq!(DownloadFormat::parse("rdf"), Some(DownloadFormat::Rdf));
    assert_eq!(DownloadFormat::parse("turtle"), None);
}
