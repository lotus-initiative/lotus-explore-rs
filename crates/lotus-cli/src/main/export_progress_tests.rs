// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The visual progress bar and table count shown while the exporters fetch
//! sequential requests.

use super::ExportProgress;

#[test]
fn the_visible_export_bar_and_count_advance_between_tables() {
    let mut progress = ExportProgress::new(
        Vec::new(),
        true,
        lotus_explore_core::export::pipeline::EXPORT_QUERY_COUNT,
    );

    progress.step("compound_taxon_reference");
    let step_0 = "\r\x1b[2K[\x1b[36m>\x1b[34m----------------------------------------\x1b[0m] 0/4 Fetching compound_taxon_reference";
    assert_eq!(
        String::from_utf8_lossy(&progress.writer),
        step_0,
        "the first request starts with an empty bar and 0/4 completed"
    );

    progress.step("compounds");
    let step_1 = format!(
        "{step_0}\r\x1b[2K[\x1b[36m==========>\x1b[34m------------------------------\x1b[0m] 1/4 Fetching compounds"
    );
    assert_eq!(
        String::from_utf8_lossy(&progress.writer),
        step_1,
        "the bar fills to 10/40 and advances to 1/4 while the second request is active"
    );

    progress.step("taxa");
    let step_2 = format!(
        "{step_1}\r\x1b[2K[\x1b[36m====================>\x1b[34m--------------------\x1b[0m] 2/4 Fetching taxa"
    );
    assert_eq!(
        String::from_utf8_lossy(&progress.writer),
        step_2,
        "the bar fills to 20/40 and advances to 2/4 while the third request is active"
    );

    progress.step("references");
    let step_3 = format!(
        "{step_2}\r\x1b[2K[\x1b[36m==============================>\x1b[34m----------\x1b[0m] 3/4 Fetching references"
    );
    assert_eq!(
        String::from_utf8_lossy(&progress.writer),
        step_3,
        "the bar fills to 30/40 and advances to 3/4 while the fourth request is active"
    );

    progress.finish();
    assert_eq!(
        String::from_utf8_lossy(&progress.writer),
        format!("{step_3}\r\x1b[2K"),
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
