// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The in-memory fallback has a row ceiling, and this is the policy behind it.
//!
//! It lives here rather than in the wasm-only download module for the reason the
//! sink order does: a policy whose tests never run is not a policy.

#[test]
fn the_ceiling_follows_the_format_and_not_one_number() {
    // A row count cannot be right for all three formats: the same row is a flat
    // CSV line, a JSON object with its keys spelled out, or the reified Turtle
    // chain. One number either starves CSV or hands Turtle a budget that takes
    // the tab with it.
    let csv = super::blob_path_ceiling(super::DownloadFormat::Csv);
    let json = super::blob_path_ceiling(super::DownloadFormat::Json);
    let rdf = super::blob_path_ceiling(super::DownloadFormat::Rdf);
    assert_eq!(
        csv,
        super::DOWNLOAD_MAX_ROWS,
        "CSV is the format the budget fits, and the row ceiling is what binds"
    );
    assert!(
        json < csv,
        "an object per row cannot be cheaper than a CSV line: {json} vs {csv}"
    );
    assert!(
        rdf <= csv,
        "Turtle cannot cost more per row than CSV: {rdf} vs {csv}"
    );
}

/// Turtle is cheaper per row than JSON, which the estimates got backwards.
///
/// This used to assert `rdf < json` on the reasoning that "eight triples cannot be
/// cheaper than one object". Measured, that is false: the emitter writes the same
/// handful of short IRIs per row, where JSON spells out a key per column, and
/// Turtle comes to 276 B/row against JSON's 667. So Turtle gets the *higher*
/// ceiling.
///
/// Recorded as its own assertion because the inversion is the kind of thing a
/// reader would assume was a bug in the exporter rather than in an estimate: if
/// Turtle ever does start costing more than JSON, this is where it shows up.
#[test]
fn turtle_is_cheaper_per_row_than_json_because_the_estimates_used_to_say_otherwise() {
    let json = super::blob_path_ceiling(super::DownloadFormat::Json);
    let rdf = super::blob_path_ceiling(super::DownloadFormat::Rdf);
    assert!(
        rdf > json,
        "Turtle measured 276 B/row against JSON's 667, so it earns the higher \
         ceiling; if this has inverted, the exporter changed and BYTES_PER_ROW has \
         to be re-measured rather than adjusted"
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
    // Per-row cost, not ceiling: an estimate that reorders would make the gate
    // wrong in the direction nobody notices, a format charged too little handed a
    // budget it cannot hold. Ceiling is the inverse of cost and is bounded by
    // `DOWNLOAD_MAX_ROWS`, so CSV and Turtle both land on the same ceiling while
    // costing very different amounts, and ordering the ceilings would say nothing.
    let cost = |f: super::DownloadFormat| match f {
        super::DownloadFormat::Csv => 116,
        super::DownloadFormat::Json => 667,
        super::DownloadFormat::Rdf => 276,
    };
    assert!(
        cost(super::DownloadFormat::Csv) < cost(super::DownloadFormat::Json),
        "a CSV line is the narrowest of the three"
    );
    assert!(
        cost(super::DownloadFormat::Rdf) < cost(super::DownloadFormat::Json),
        "measured: Turtle writes short repeated IRIs where JSON spells out a key \
         per column, so Turtle is the cheaper of the two despite emitting more lines"
    );
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
