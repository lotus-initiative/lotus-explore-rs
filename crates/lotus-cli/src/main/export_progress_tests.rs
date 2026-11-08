// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The table count shown while the bulk exporter fetches sequential requests.

use super::ExportProgress;

#[test]
fn the_visible_export_count_advances_between_tables() {
    let mut progress = ExportProgress::new(
        Vec::new(),
        true,
        lotus_explore_core::export::pipeline::EXPORT_QUERY_COUNT,
    );

    progress.step("compound_taxon_reference");
    assert_eq!(
        String::from_utf8_lossy(&progress.writer),
        "\r\x1b[2K0/4 Fetching compound_taxon_reference",
        "the first request has started, but none has completed"
    );

    progress.step("compounds");
    assert_eq!(
        String::from_utf8_lossy(&progress.writer),
        "\r\x1b[2K0/4 Fetching compound_taxon_reference\r\x1b[2K1/4 Fetching compounds",
        "the first request must count as completed while the second is active"
    );

    progress.step("taxa");
    assert_eq!(
        String::from_utf8_lossy(&progress.writer),
        "\r\x1b[2K0/4 Fetching compound_taxon_reference\r\x1b[2K1/4 Fetching compounds\r\x1b[2K2/4 Fetching taxa",
        "the completed count must continue advancing"
    );

    progress.finish();
    assert!(
        String::from_utf8_lossy(&progress.writer).ends_with("\r\x1b[2K"),
        "finishing clears the transient status line"
    );
}

#[test]
fn non_terminal_stderr_emits_no_progress_escapes() {
    let mut progress = ExportProgress::new(
        Vec::new(),
        false,
        lotus_explore_core::export::pipeline::EXPORT_QUERY_COUNT,
    );
    progress.step("compound_taxon_reference");
    progress.finish();
    assert!(
        progress.writer.is_empty(),
        "non-TTY stderr must stay quiet so piped logs are not polluted with ANSI escapes"
    );
}
