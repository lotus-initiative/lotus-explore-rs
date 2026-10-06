// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The in-memory fallback has a row ceiling, and this is the policy behind it.
//!
//! It lives here rather than in the wasm-only download module for the reason the
//! sink order does: a policy whose tests never run is not a policy.

#[test]
fn the_ceiling_follows_the_format_and_not_one_number() {
    // A row count cannot be right for all three formats: the same row is a flat
    // CSV line, a JSON object with its keys spelled out, or eight Turtle triples
    // carrying the reified statement chain. One number either starves CSV or hands
    // Turtle a budget that takes the tab with it.
    let csv = super::blob_path_ceiling(super::DownloadFormat::Csv);
    let json = super::blob_path_ceiling(super::DownloadFormat::Json);
    let rdf = super::blob_path_ceiling(super::DownloadFormat::Rdf);
    assert_eq!(
        csv,
        super::DOWNLOAD_MAX_ROWS,
        "CSV is the format the budget fits exactly"
    );
    assert!(
        json < csv,
        "an object per row cannot be cheaper than a CSV line"
    );
    assert!(
        rdf < json,
        "eight triples cannot be cheaper than one object"
    );
}

#[test]
fn the_csv_ceiling_is_a_million_rows() {
    // A `const` block, as the assertions in `table_budget` do: both operands are
    // constants, so this checks the relationship at compile time rather than
    // re-deriving a literal at every test run, and a const block cannot format,
    // which is why the message is a static string.
    const _: () = assert!(
        super::CSV_BYTES_PER_ROW * super::DOWNLOAD_MAX_ROWS <= super::IN_MEMORY_BUDGET_BYTES,
        "a million CSV rows must fit inside the budget the gate enforces, or the gate \
         refuses the very export it was sized for"
    );
    assert_eq!(super::DOWNLOAD_MAX_ROWS, 1_000_000);
}

#[test]
fn the_estimates_are_ordered_the_way_the_formats_nest() {
    // An estimate that reorders would make the gate wrong in the direction nobody
    // notices: a format charged too little would be handed a budget it cannot hold.
    let bytes = |f: super::DownloadFormat| super::blob_path_ceiling(f);
    assert!(bytes(super::DownloadFormat::Csv) >= bytes(super::DownloadFormat::Json));
    assert!(bytes(super::DownloadFormat::Json) >= bytes(super::DownloadFormat::Rdf));
}

#[test]
fn the_in_memory_path_refuses_past_the_ceiling() {
    // The failure this prevents is a killed tab, not an error message.
    assert!(super::blob_path_can_carry(0, super::DownloadFormat::Csv));
    assert!(super::blob_path_can_carry(
        10_000,
        super::DownloadFormat::Csv
    ));
    assert!(
        super::blob_path_can_carry(super::DOWNLOAD_MAX_ROWS, super::DownloadFormat::Csv),
        "the ceiling itself must be allowed"
    );
    assert!(
        !super::blob_path_can_carry(super::DOWNLOAD_MAX_ROWS + 1, super::DownloadFormat::Csv),
        "one row past the ceiling must be refused"
    );
}

#[test]
fn the_message_names_the_browser_limit_and_the_way_out() {
    let limit = super::DOWNLOAD_MAX_ROWS;
    let message = super::blob_path_message(limit + 1, limit);
    assert!(message.contains(&format!("{limit}")), "{message}");
    assert!(
        message.contains("disk"),
        "a reader who cannot act on the limit needs to be told what would lift it: \
         {message}"
    );
}
