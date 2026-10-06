// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Folding a streamed body into a result set, and the two input resolvers.
//!
//! All three run against [`Scripted`], so nothing here opens a socket. The
//! streaming tests hand the body over in deliberately awkward pieces, because a
//! whole-body double proves nothing about a reader that has to carry a record
//! across a chunk boundary -- which is the only reason the streaming path exists.

// The panic lints keep library code free of panics on external input. A test
// failing on a bad fixture is reporting, not panicking.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus_search::testing::Scripted;
use lotus_search::{
    ChunkedBody, FetchError, ResponseFormat, StreamProgress, columnar_from_chunks_reporting,
    execute_streaming_with_fallback, resolve_reference, resolve_structure,
};

/// The result CSV the endpoint produces, with one value containing a comma and a
/// quote, so the reader's quoting is exercised rather than assumed.
const RESULT_CSV: &str = "compound,compoundLabel,compound_mass,taxon,ref_qid\n\
                           Q1,\"Aspirin, plain\",120.5,Q10,Q100\n\
                           Q4,Salicylic,138.1,Q10,Q100\n";

const QUERY: &str = "SELECT ?compound WHERE { ?compound wdt:P31 wd:Q11365 }";

/// Ask the scripted transport for `body`, handed over `chunk` bytes at a time.
async fn streamed_body(body: &str, chunk: usize) -> ChunkedBody {
    let http = Scripted::with_chunk_size(vec![(200, body)], chunk);
    execute_streaming_with_fallback(&http, QUERY, ResponseFormat::Csv)
        .await
        .expect("the scripted transport answers")
        .chunks
}

/// Fold `csv` through the streaming reader, returning the row count and every
/// progress report made along the way.
async fn fold(csv: &str, chunk: usize) -> (usize, Vec<StreamProgress>) {
    let mut reports = Vec::new();
    let body = streamed_body(csv, chunk).await;
    let set = columnar_from_chunks_reporting(body, &mut |p| reports.push(p))
        .await
        .expect("the CSV parses");
    (set.row_count(), reports)
}

#[tokio::test]
async fn a_record_split_across_a_chunk_boundary_is_one_row() {
    // One byte at a time is the worst case the reader can be handed: every
    // record straddles several chunks and every field straddles a boundary.
    for chunk in [1usize, 7, 13, 64, 4096] {
        let (rows, _) = fold(RESULT_CSV, chunk).await;
        assert_eq!(
            rows, 2,
            "chunk size {chunk} produced {rows} rows: the boundary handling is wrong"
        );
    }
}

#[tokio::test]
async fn progress_is_reported_once_per_chunk_and_never_goes_backwards() {
    let (rows, reports) = fold(RESULT_CSV, 16).await;
    assert_eq!(rows, 2, "the fold and the reports must see the same rows");

    // One report per chunk, and the last has to have seen everything: a caller
    // that throttles its re-render on this number would otherwise stop short.
    assert!(
        reports.len() > 1,
        "a 16-byte chunk over {} bytes should report more than once, got {}",
        RESULT_CSV.len(),
        reports.len()
    );
    for pair in reports.windows(2) {
        assert!(
            pair[1].rows >= pair[0].rows && pair[1].bytes >= pair[0].bytes,
            "progress went backwards: {:?} then {:?}",
            pair[0],
            pair[1]
        );
    }
    let last = reports.last().expect("at least one report");
    assert_eq!(last.rows, 2, "the final report is not the final row count");
    assert!(
        last.bytes >= RESULT_CSV.len(),
        "the final byte count {} is short of the body's {}",
        last.bytes,
        RESULT_CSV.len()
    );
}

#[tokio::test]
async fn an_empty_body_is_an_empty_set_and_not_an_error() {
    // Headers only: a query that matched nothing is a real answer, and the
    // caller draws an empty table rather than an error panel.
    let (rows, _) = fold("compound,taxon,ref_qid\n", 8).await;
    assert_eq!(rows, 0);
}

#[tokio::test]
async fn a_body_that_is_not_the_expected_csv_is_a_parse_error() {
    // No recognisable header at all. The distinction matters: a transport that
    // answers 200 with an error page must not read as "no results", and the
    // error has to be `Parse` rather than `Network` -- the request succeeded.
    let body = streamed_body("<html>gateway timeout</html>", 16).await;
    let error = columnar_from_chunks_reporting(body, &mut |_| {})
        .await
        .expect_err("an HTML body is not a result set");
    assert!(
        matches!(error, FetchError::Parse(_)),
        "expected a parse error, got {error:?}"
    );
}

#[tokio::test]
async fn a_bare_qid_resolves_without_a_request() {
    // No lookup: a caller typing a QID has already given the answer, so a round
    // trip could only fail. Uppercased, because `q23118` and `Q23118` are the
    // same item and the rest of the app emits the upper-case form.
    let http = Scripted::new(vec![(200, "should never be read")]);
    let resolution = resolve_structure(&http, "  q23118  ")
        .await
        .expect("a bare QID resolves");
    assert_eq!(resolution.compounds, vec!["Q23118".to_string()]);
    assert_eq!(http.call_count(), 0, "a bare QID must not send a request");
}

#[tokio::test]
async fn a_compound_name_falls_back_to_its_alias_when_the_label_matches_nothing() {
    // The label query answers with a header and no rows; only the alias query
    // finds it. Both are made: an empty result is also what a misspelt name
    // gives, so the label query cannot be abandoned on emptiness.
    let empty = "compound_qid,compound_label,canonical_smiles,matched_by\n";
    let found = format!("{empty}Q9,Amarogentina,CCO,alias\n");
    let http = Scripted::new(vec![(200, empty), (200, &found)]);
    let resolution = resolve_structure(&http, "amarogentina")
        .await
        .expect("the alias match resolves");
    assert_eq!(
        resolution.compounds,
        vec!["Q9".to_string()],
        "the alias match must be taken when the label matched nothing"
    );
    assert_eq!(http.call_count(), 2, "both queries run, in order");
}

#[tokio::test]
async fn a_reference_resolves_from_a_qid_without_a_lookup() {
    let http = Scripted::new(vec![(200, "should never be read")]);
    let resolved = resolve_reference(&http, " q12345 ")
        .await
        .expect("a bare QID resolves");
    assert_eq!(resolved.as_deref(), Some("Q12345"));
    assert_eq!(http.call_count(), 0, "a QID must not be looked up");

    // Empty is neither an error nor a lookup: there is nothing to resolve, and
    // the caller distinguishes "no reference given" from "not found".
    let http = Scripted::new(vec![(200, "should never be read")]);
    assert_eq!(
        resolve_reference(&http, "   ")
            .await
            .expect("empty input is not an error"),
        None
    );
    assert_eq!(http.call_count(), 0, "empty input must not be looked up");
}

#[tokio::test]
async fn a_doi_that_matched_nothing_is_an_error_and_not_an_empty_result() {
    // The user asked about a specific reference and Wikidata does not have it.
    // `Ok(None)` here would read as "no reference was given".
    let http = Scripted::new(vec![(200, "reference,reference_label\n")]);
    let error = resolve_reference(&http, "10.1000/nope")
        .await
        .expect_err("a DOI that matched nothing is not a reference");
    assert!(
        matches!(error, FetchError::Parse(_)),
        "expected a parse error, got {error:?}"
    );
}
