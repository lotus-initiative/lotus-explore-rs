// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Source-level invariants for `handlers::export`.

const HANDLERS: &str = include_str!("export/stream.rs");

/// The export path must never ask `QLever` for a `CONSTRUCT`.
#[test]
fn the_export_never_asks_the_endpoint_for_a_construct() {
    let prepared = concat!("prepared_", "query");
    assert!(
        !HANDLERS.contains(prepared),
        "the export path must render Turtle locally, not ask for the CONSTRUCT \
         that wrapping implies"
    );
    assert!(
        HANDLERS.contains("ExportFormat::Csv.qlever_action()")
            && HANDLERS.contains("ResponseFormat::Csv.accept()"),
        "the export must fetch the SELECT as CSV; asking for any other format upstream \
         reintroduces endpoint-side materialisation"
    );
}

/// Every non-CSV format has to be produced here, from rows already fetched.
#[test]
fn a_non_csv_format_is_built_here_rather_than_passed_through() {
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
    assert!(
        HANDLERS.contains("tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(8)"),
        "the outbound channel must be bounded, or backpressure becomes buffering"
    );
}

/// An export runs in a spawned task after its handler has returned, so the
/// permits that bound it have to move into that task.
#[test]
fn an_export_holds_its_permits_until_it_is_done() {
    let handler = HANDLERS
        .split_once("pub async fn export_file(")
        .and_then(|(_, rest)| rest.split_once("\nasync fn pump_export"))
        .map_or("", |(body, _)| body);
    assert!(
        handler.contains(concat!("try_acquire", "_owned()")),
        "the request permit must be owned, so it can outlive the handler"
    );
    assert!(
        handler.contains(concat!("acquire_upstream", "_permit(")),
        "an export is an upstream query and takes an upstream slot"
    );
    assert!(
        handler.contains(concat!("let _held = (permit, upstream", "_permit);")),
        "both permits move into the task that does the export"
    );
}
