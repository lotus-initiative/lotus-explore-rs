// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `handlers`, in their own file.

const HANDLERS: &str = include_str!("../handlers.rs");

/// The export path must never ask `QLever` for a `CONSTRUCT`.
///
/// This is the whole point of the change and it is invisible in the response: a
/// `CONSTRUCT` works, returns correct Turtle, and is simply capped at whatever
/// the endpoint will materialise inside 30 seconds. Nothing fails when the cap
/// is hit except an export that is quietly smaller than the one asked for, which
/// is the failure mode a reader is least likely to notice and most likely to
/// spend hours on.
#[test]
fn the_export_never_asks_the_endpoint_for_a_construct() {
    // The method that wraps a SELECT in a CONSTRUCT for the Rdf format is reachable
    // only through the URL builder, and only bites when handed that format. Called
    // directly here with Rdf, the cap is back.
    //
    // The needles are assembled at compile time rather than written out: this
    // module is part of the file it searches, so a literal would match its own
    // assertion and the gate would pass forever.
    let prepared = concat!("prepared_", "query");
    assert!(
        !HANDLERS.contains(prepared),
        "the export path must render Turtle locally, not ask for the CONSTRUCT \
         that wrapping implies"
    );
    assert!(
        HANDLERS.contains("qlever_export_url(&cached.query, ExportFormat::Csv)"),
        "the export must fetch the SELECT; asking for any other format upstream \
         reintroduces endpoint-side materialisation"
    );
}

/// Every non-CSV format has to be produced here, from rows already fetched.
///
/// A `match` that handled CSV and then passed the upstream bytes through
/// unchanged for the rest would type-check, pass a content-type test, and hand a
/// reader a file called `.ttl` containing CSV.
#[test]
fn a_non_csv_format_is_built_here_rather_than_passed_through() {
    // Split for the same reason: both needles appear in this module's own text.
    let reader = concat!("CsvColumnar", "Reader");
    let exporter = concat!("RowExporter::", "new(format, &set)");
    assert!(
        HANDLERS.contains(reader),
        "the rows have to be parsed before they can be re-rendered"
    );
    assert!(
        HANDLERS.contains(exporter),
        "every format but CSV is rendered locally from the fetched rows"
    );
}

/// The export must not buffer the upstream body, and must not build the whole
/// output before sending any of it.
///
/// This is the difference between peak memory being one chunk and peak memory
/// being the export twice over: the bulk fetch helper returns a `Vec<u8>` of the
/// entire body, and a gzip encoder over a `Vec` holds the compressed copy alongside.
/// Both are invisible in a response that succeeds, so the shape is pinned here
/// rather than measured. `next_chunk` is what makes the output incremental --
/// `for_each_chunk` would have needed the whole render to finish first, because
/// it is synchronous and cannot await a send.
#[test]
fn the_export_streams_rather_than_buffering() {
    let buffered = concat!("fetch_", "url");
    let encoder = concat!("GzEncoder::new(Vec::new()", ", Compression::default())");
    assert!(
        !HANDLERS.contains(buffered),
        "the export must read the upstream chunk by chunk; fetching it whole puts \
         the entire export in memory before the first byte is sent"
    );
    assert!(
        HANDLERS.contains(encoder),
        "gzip must be incremental, or the compressed copy doubles peak memory"
    );
    let per_chunk = concat!("RowExporter::", "new(format, &set)");
    let drain = concat!("exporter.next_", "chunk()");
    assert!(
        HANDLERS.contains(per_chunk) && HANDLERS.contains(drain),
        "the rendered output must be drained a chunk at a time"
    );
    // The channel is what stops a client that stops reading from being turned
    // into unbounded buffering somewhere less visible than the export.
    assert!(
        HANDLERS.contains("tokio::sync::mpsc::channel::<Bytes>(8)"),
        "the outbound channel must be bounded, or backpressure becomes buffering"
    );
}
