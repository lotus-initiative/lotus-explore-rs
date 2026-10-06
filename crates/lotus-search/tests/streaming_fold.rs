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
    ChunkedBody, FetchError, ResponseFormat, StreamProgress, columnar_from_chunks,
    columnar_from_chunks_reporting, execute_streaming_with_fallback, resolve_reference,
    resolve_structure,
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

#[tokio::test]
async fn the_plain_wrapper_folds_the_same_set_as_the_reporting_one() {
    // `columnar_from_chunks` is the public entry point -- the reporting variant
    // is the one the app uses, because the app has a `Signal` to update -- and it
    // had no test at all. That let a mutant replace its whole body with an empty
    // set and survive: nothing called it, so nothing noticed.
    //
    // What it owes a caller is that dropping the progress callback changes
    // nothing else: the same body in, the same rows out.
    let plain = columnar_from_chunks(streamed_body(RESULT_CSV, 32).await)
        .await
        .expect("the CSV parses");
    let mut reports = 0usize;
    let reporting =
        columnar_from_chunks_reporting(streamed_body(RESULT_CSV, 32).await, &mut |_| {
            reports += 1;
        })
        .await
        .expect("the CSV parses");

    assert_eq!(
        plain.row_count(),
        reporting.row_count(),
        "the wrapper and the reporting variant must fold the same rows"
    );
    assert_eq!(plain.row_count(), 2, "which is the two rows in the fixture");
    assert_eq!(
        plain.stats().n_compounds,
        reporting.stats().n_compounds,
        "and the same statistics, so the wrapper is not quietly a different query"
    );
    assert!(
        reports > 1,
        "the reporting variant still reported per chunk, so this is a real comparison"
    );
}

/// A truncated export is refused, and the reason is the endpoint's own.
///
/// `QLever` cannot report a failure that happens after the `200` is committed, so
/// it writes the error into the body of the answer. Without reading it, a result
/// cut at a row boundary is a valid CSV file with fewer rows in it, and the row
/// count the UI shows is then a guess it cannot tell from an answer.
///
/// The marker and the prose are theirs; the payload below is the tail of a real
/// response captured from `qlever.dev`, because a hand-written fixture would only
/// prove that the parser agrees with itself.
const TRUNCATED_TAIL: &str = "Q4,Salicylic,138.1,Q10,Q100\n\
Q7,Paracetamol,151.1,Q10,Q100\n\
!!!!>># An error has occurred while exporting the query result. Unfortunately due \
to limitations in the HTTP 1.1 protocol, there is no better way to report this \
than to append it to the incomplete result. The error message was:\n\
Operation timed out.\n";

/// Parse `body`, returning the error message on failure.
async fn parse_or_message(body: &str, chunk: usize) -> Result<usize, String> {
    let chunks = streamed_body(body, chunk).await;
    columnar_from_chunks(chunks)
        .await
        .map(|set| set.row_count())
        .map_err(|e| e.to_string())
}

#[tokio::test]
async fn a_truncated_export_is_refused_rather_than_parsed() {
    for chunk in [1usize, 7, 64, 512, 4096] {
        let body = format!("{RESULT_CSV}{TRUNCATED_TAIL}");
        let error = parse_or_message(&body, chunk)
            .await
            .expect_err("a truncated answer must not parse");
        assert!(
            error.contains("stopped sending"),
            "chunk size {chunk} gave {error:?}, which does not say the answer was cut"
        );
        assert!(
            error.contains("Operation timed out"),
            "the endpoint's own reason must survive into the message: {error:?}"
        );
    }
}

#[tokio::test]
async fn the_same_body_without_the_notice_is_the_answer_it_claims_to_be() {
    // The other half: the check has to be narrow, or it refuses good results.
    // The two bodies differ only in the notice.
    let complete = RESULT_CSV;
    let rows = parse_or_message(complete, 64)
        .await
        .expect("a complete answer parses");
    assert_eq!(rows, 2);

    let truncated = format!("{RESULT_CSV}{TRUNCATED_TAIL}");
    assert!(
        parse_or_message(&truncated, 64).await.is_err(),
        "the notice has to change the outcome"
    );
}

#[tokio::test]
async fn a_notice_split_across_a_chunk_boundary_is_still_seen() {
    // The notice arrives at the very end and can straddle a boundary. A tail
    // window smaller than the notice would read a cut body as complete, which is
    // the failure this whole check exists to prevent.
    let body = format!("{RESULT_CSV}{TRUNCATED_TAIL}");
    let error = parse_or_message(&body, 13)
        .await
        .expect_err("the notice is found whatever the chunking");
    assert!(error.contains("Operation timed out"), "{error:?}");
}

/// The structure to search with, chosen from a name that resolved.
///
/// A name typed into the structure field resolves to a compound, and the search
/// then runs against *that compound's* canonical SMILES rather than the text the
/// user typed. Two rules in there had no test: a match with no SMILES is skipped
/// rather than taken, and an empty match list leaves the user's own text alone.
#[tokio::test]
async fn a_name_that_resolves_is_searched_by_the_compounds_own_smiles() {
    let found = "compound_qid,compound_label,canonical_smiles,matched_by\n\
                 Q9,Amarogentina,COCC1OC(O)C(O)C(O)C1O,label\n";
    let http = Scripted::new(vec![(200, found)]);
    let resolution = resolve_structure(&http, "amarogentina")
        .await
        .expect("the name resolves");

    assert_eq!(
        resolution.smiles, "COCC1OC(O)C(O)C(O)C1O",
        "the search must run against the compound's structure, not the name"
    );
    assert_eq!(
        resolution.looked_up, "amarogentina",
        "and the input is reported as typed"
    );
}

#[tokio::test]
async fn a_match_with_no_smiles_is_skipped_for_one_that_has_one() {
    // Several stereoisomers can carry the label. The first one with a structure
    // wins -- taking the first match and its empty SMILES would hand an empty
    // string to the structure service, which is a request that cannot succeed.
    let found = "compound_qid,compound_label,canonical_smiles,matched_by\n\
                 Q1,Amarogentina,,label\n\
                 Q2,Amarogentina,CCO,label\n\
                 Q3,Amarogentina,CCC,label\n";
    let http = Scripted::new(vec![(200, found)]);
    let resolution = resolve_structure(&http, "amarogentina")
        .await
        .expect("the name resolves");

    assert_eq!(
        resolution.smiles, "CCO",
        "the first match that HAS a structure wins, not the first match"
    );
    assert_eq!(
        resolution.compounds,
        vec!["Q1".to_string(), "Q2".to_string(), "Q3".to_string()],
        "every match is kept, because a cutoff of 1.0 legitimately returns several"
    );
}

#[tokio::test]
async fn a_name_that_resolves_to_nothing_keeps_the_users_own_text() {
    // Nothing matched, so there is no compound to take a structure from. The
    // input stands: passing an empty string to the structure service would be a
    // request for nothing.
    let empty = "compound_qid,compound_label,canonical_smiles,matched_by\n";
    let http = Scripted::new(vec![(200, empty), (200, empty)]);
    let resolution = resolve_structure(&http, "amarogentina")
        .await
        .expect("an unresolved name is not an error here");

    assert!(
        resolution.compounds.is_empty(),
        "nothing matched, so there are no compounds"
    );
    assert_eq!(
        resolution.smiles, "amarogentina",
        "and the user's own text is what would be searched"
    );
}
