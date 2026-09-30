// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the structure conversion, in their own file.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use super::*;
use lotus_search::testing::Scripted;

/// Build a transport from replies made at runtime.
///
/// They leak: a test process is short-lived, and a fixture that must be
/// spelled as one long literal is a fixture nobody can read.
fn script(rows: Vec<(u16, String)>) -> Scripted {
    Scripted::new(
        rows.into_iter()
            .map(|(status, body)| (status, &*Box::leak(body.into_boxed_str())))
            .collect(),
    )
}

/// A successful conversion row, as the service writes it.
fn ok(output: &str) -> String {
    format!(r#"{{"success":true,"output":"{output}","error":""}}"#)
}

/// A whole batch reply, one successful row per output.
fn batch(outputs: &[&str]) -> String {
    let rows: Vec<String> = outputs.iter().map(|output| ok(output)).collect();
    format!(r#"{{"results":[{}]}}"#, rows.join(","))
}

/// The three replies a full conversion takes, for one structure.
fn one_structure() -> Vec<(u16, String)> {
    vec![
        (200, batch(&["LFQSCWFLJHTTHZ-UHFFFAOYSA-N"])),
        (200, batch(&["CCO"])),
        (200, batch(&["InChI=1S/C2H6O/c1-2-3/h3H,2H2,1H3"])),
    ]
}

/// A batch in which the first structure is unreadable and the second is not.
fn batch_with_one_bad() -> Vec<(u16, String)> {
    vec![
            (
                200,
                r#"{"results":[{"success":false,"output":"","error":"unparsable"},{"success":true,"output":"KEY2","error":""}]}"#
                    .to_string(),
            ),
            (200, batch(&["CCO", "CCC"])),
        ]
}

#[tokio::test]
async fn a_structure_becomes_the_key_wikidata_is_matched_on() {
    let http = script(one_structure());
    let structure = convert_structure(&http, "CCO")
        .await
        .expect("the conversion succeeds");

    assert_eq!(
        structure.inchikey.as_deref(),
        Some("LFQSCWFLJHTTHZ-UHFFFAOYSA-N")
    );
    assert!(structure.is_known());
    assert_eq!(
        structure.structure_key().inchikey.as_deref(),
        Some("LFQSCWFLJHTTHZ-UHFFFAOYSA-N")
    );
}

#[tokio::test]
async fn a_whole_file_costs_three_requests_and_not_three_per_row() {
    // Why this is batched: the per-row version was rate-limited on the third
    // row of a five-row file. This is the assertion that keeps it that way.
    let http = script(vec![
        (200, batch(&["A", "B", "C", "D", "E"])),
        (200, batch(&["CCO", "CCC", "CCN", "CCC", "CCO"])),
        (200, batch(&["i1", "i2", "i3", "i4", "i5"])),
    ]);

    let rows = ["CCO", "CCC", "CCN", "CCC", "CCO"];
    let converted = convert_structures(&http, &rows)
        .await
        .expect("the file converts");

    assert_eq!(converted.len(), 5);
    assert_eq!(http.call_count(), 3, "three requests for the whole file");
}

#[tokio::test]
async fn every_structure_is_sent_in_one_request() {
    let http = script(vec![
        (200, batch(&["A", "B"])),
        (200, batch(&["CCO", "CCC"])),
        (200, batch(&["x", "y"])),
    ]);

    let _ = convert_structures(&http, &["CCO", "CCC"]).await;

    let asked = http.queries();
    assert!(asked[0].contains(r#""CCO""#), "{asked:?}");
    assert!(
        asked[0].contains(r#""CCC""#),
        "both structures in one request: {asked:?}"
    );
}

#[tokio::test]
async fn the_key_is_asked_for_first_and_there_is_no_isomeric_output() {
    let http = script(one_structure());
    let _ = convert_structure(&http, "CCO").await;

    let asked = http.queries();
    assert!(asked[0].contains("output_format=inchikey"), "{asked:?}");
    assert!(
        asked[1].contains("output_format=canonicalsmiles"),
        "{asked:?}"
    );
    // The service has no isomeric-SMILES output; asking would fail.
    assert!(
        !asked.iter().any(|call| call.contains("isomeric")),
        "the service has no isomeric output: {asked:?}"
    );
}

#[tokio::test]
async fn the_structures_are_sent_as_smiles_and_trimmed() {
    let http = script(one_structure());
    let _ = convert_structure(&http, " CCO ").await;

    // A spreadsheet cell with a stray space still converts.
    assert!(
        http.queries()[0].contains(r#""CCO""#),
        "{:?}",
        http.queries()
    );
}

#[tokio::test]
async fn a_structure_the_service_cannot_read_is_invalid_input() {
    let http = script(batch_with_one_bad());

    let err = convert_structures(&http, &["bad", "CCO"])
        .await
        .expect_err("the unreadable structure is reported");

    assert!(
        matches!(err, CurationError::InvalidInput(ref m) if m.contains("unparsable")),
        "{err:?}"
    );
    // A fault in the row, so retrying changes nothing.
    assert!(!err.is_recoverable(), "{err:?}");
}

#[tokio::test]
async fn a_reply_that_is_short_is_a_parse_error_not_a_missing_row() {
    // If the service answers for fewer structures than it was sent, the
    // missing ones must not read as "this structure is absent" -- that is how
    // a compound nobody looked up gets reported as new.
    let http = script(vec![(200, batch(&["A"]))]);

    let err = convert_structures(&http, &["CCO", "CCC"])
        .await
        .expect_err("a short reply is not an answer for the file");

    assert!(matches!(err, CurationError::Parse(_)), "{err:?}");
    assert!(err.to_string().contains("1 of 2"), "{err}");
}

#[tokio::test]
async fn an_empty_structure_is_refused_before_any_request() {
    let http = script(vec![]);
    let err = convert_structure(&http, "   ")
        .await
        .expect_err("there is nothing to convert");
    assert!(matches!(err, CurationError::InvalidInput(_)), "{err:?}");
    assert_eq!(
        http.call_count(),
        0,
        "no request for a row with no structure"
    );
}

#[tokio::test]
async fn a_key_that_comes_back_empty_is_not_a_key() {
    // An empty output is how the service says "I have no answer", and
    // treating it as a key would query Wikidata for the empty string.
    let http = script(vec![(200, batch(&[""]))]);

    let err = convert_structure(&http, "CCO")
        .await
        .expect_err("an empty output is not a key");

    assert!(matches!(err, CurationError::Parse(_)), "{err:?}");
}

#[tokio::test]
async fn a_rejected_conversion_is_a_transport_error_naming_the_rate_limit() {
    let http = script(vec![(429, "slow down".into())]);
    let err = convert_structure(&http, "CCO")
        .await
        .expect_err("a 429 is not a conversion");
    assert!(matches!(err, CurationError::Http(_)), "{err:?}");
    assert!(
        err.is_recoverable(),
        "a 429 is what a retry is for: {err:?}"
    );
    // The one status a user can act on by asking for less.
    assert!(err.to_string().contains("rate-limiting"), "{err}");
}

#[tokio::test]
async fn a_missing_inchi_does_not_fail_the_conversion() {
    // The `InChI` is a bonus; the key is what the run turns on.
    let http = script(vec![
        (200, batch(&["LFQSCWFLJHTTHZ-UHFFFAOYSA-N"])),
        (200, batch(&["CCO"])),
        (500, "no".into()),
    ]);

    let structure = convert_structure(&http, "CCO")
        .await
        .expect("the run continues");

    assert!(structure.is_known());
    assert_eq!(
        structure.inchi, None,
        "the InChI is optional, the key is not"
    );
}

#[tokio::test]
async fn no_structures_is_refused_rather_than_answered_with_nothing() {
    let http = script(vec![]);
    let err = convert_structures(&http, &[])
        .await
        .expect_err("an empty file is not a conversion");
    assert!(matches!(err, CurationError::InvalidInput(_)), "{err:?}");
    assert_eq!(http.call_count(), 0);
}

#[test]
fn a_success_with_no_output_is_a_parse_failure() {
    // The service answered, and what it answered was not a structure. That is
    // a shape this does not read, so it is reported as a parse failure rather
    // than quietly curating an empty value.
    for blank in ["", "   ", "\n\t "] {
        let converted = Converted {
            output: Some(blank.to_owned()),
            error: None,
        };
        let result = converted.into_result("SMILES");
        assert!(
            matches!(result, Err(CurationError::Parse(ref message)) if message.contains("no SMILES")),
            "{blank:?} is not an output, got {result:?}"
        );
    }
}

#[test]
fn a_real_output_is_trimmed_and_kept() {
    let result = Converted {
        output: Some("  CCO  ".to_owned()),
        error: None,
    }
    .into_result("SMILES");
    assert_eq!(result.unwrap_or_default().as_deref(), Some("CCO"));
}
