// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `export`, in their own file.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use super::ExportFormat;

// The log name lands in log lines, metric labels and filenames, so a shared
// or empty name makes one export indistinguishable from another in a log the
// only way anyone will read about a failed download.
#[test]
fn each_format_has_its_own_log_name() {
    assert_eq!(ExportFormat::Csv.log_name(), "csv");
    assert_eq!(ExportFormat::Json.log_name(), "json");
    assert_eq!(ExportFormat::Rdf.log_name(), "rdf");
}

#[test]
fn the_log_name_is_the_extension_the_download_is_saved_under() {
    // The extension comes from the log name and the Content-Type from
    // `content_type`, so a mismatch is a file the browser cannot read.
    for format in [ExportFormat::Csv, ExportFormat::Json, ExportFormat::Rdf] {
        let name = format.log_name();
        assert!(!name.is_empty(), "{format:?} needs a name");
        assert!(
            format.content_type().contains(name) || name == "rdf",
            "{format:?}: {} does not describe {name}",
            format.content_type()
        );
    }
}
