// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The search sequence, driven against a scripted transport.
//!
//! This is the pipeline the web app, the CLI and the API server all call. It
//! used to exist three times over, and two of the copies disagreed about what
//! to do with a molfile similarity search, so the tests here pin the decisions
//! rather than just the happy path.

// The panic lints exist to keep library code free of panics on external input.
// A test that fails on a bad script is reporting, not panicking.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus_model::SearchCriteria;
use lotus_search::testing::{Scripted, ScriptedResponse};
use lotus_search::{FetchError, Http, SearchError, SearchRequest, search};

const NOW: u16 = 2026;
const MOLFILE: &str = "\n\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\n    0.0000    0.0000    0.0000 C   0  0  0  0  0  0\nM  END\n";

const TAXON_CSV: &str = "taxon,taxon_name\nhttp://www.wikidata.org/entity/Q16521,Gentiana lutea\n";

const ROWS_CSV: &str = "compound,compoundLabel,compound_inchikey,compound_smiles_conn,compound_smiles_iso,compound_mass,compound_formula,taxon,taxon_name,ref_qid,ref_title,ref_doi,ref_date,statement\nQ1,Quercetin,IK1,CCO,,180.16,C9H8O4,Q16521,Gentiana lutea,Q100,T,10.1/a,2021,\n";

const COUNTS_CSV: &str = "n_entries,n_entries_unique,n_compounds,n_taxa,n_references\n1,1,1,1,1\n";

/// What the identifier lookup answers: one compound, with the columns
/// `parse_compound_lookup_csv` reads.
const LOOKUP_CSV: &str =
    "compound_qid,compound_label,canonical_smiles,matched_by\n7,Berberine,COc1ccccc1O,inchikey\n";

/// The happy path: taxon lookup, rows, counts.
fn search_script() -> Scripted {
    Scripted::new(vec![(200, TAXON_CSV), (200, ROWS_CSV), (200, COUNTS_CSV)])
}

fn criteria(taxon: &str) -> SearchCriteria {
    SearchCriteria {
        taxon: taxon.into(),
        ..SearchCriteria::up_to_year(NOW)
    }
}

#[tokio::test]
async fn a_search_resolves_the_taxon_then_fetches_rows_then_counts() {
    let http = search_script();
    let request = SearchRequest::new(criteria("Gentiana lutea"), NOW);

    let result = search(&http, &request).await.expect("the search succeeds");

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].compound_qid.as_ref(), "Q1");
    assert!(!result.truncated);
    let stats = result.stats.expect("the count query answered");
    assert_eq!(stats.n_compounds, 1);

    let queries = http.queries();
    assert_eq!(queries.len(), 3, "lookup, rows, counts");
    assert!(
        queries[0].contains("wdt:P225"),
        "the first call resolves the taxon"
    );
    assert!(
        queries[1].contains("P171") && queries[1].contains("Q16521"),
        "the rows query filters on the QID the lookup returned"
    );
    assert!(
        queries[1].contains("LIMIT"),
        "and the display limit is applied"
    );
    assert!(queries[2].contains("COUNT(DISTINCT"), "the third counts");
    assert!(
        queries.iter().all(|q| q.starts_with("https://qlever.dev")),
        "all three were answered by QLever, so no fallback was needed"
    );
}

#[tokio::test]
async fn the_rows_are_still_returned_when_the_count_query_fails() {
    // The count query is the expensive one, so it is the one that gets
    // cancelled for running over. Reporting no totals would be worse than
    // reporting the ones the rows themselves support.
    let http = Scripted::new(vec![(200, TAXON_CSV), (200, ROWS_CSV)]);
    http.then_always_from(
        2,
        429,
        r#"{"exception":"Operation timed out. Last operation: Sort on ?r"}"#,
    );
    let request = SearchRequest::new(criteria("Gentiana lutea"), NOW);

    let result = search(&http, &request).await.expect("the rows are usable");

    assert_eq!(result.rows.len(), 1);
    let stats = result.stats.expect("a local count is substituted");
    assert_eq!(stats.n_entries, 1);
    assert_eq!(stats.n_compounds, 1);
}

#[tokio::test]
async fn a_query_that_ran_out_of_time_is_sent_once_and_never_retried() {
    // The load-bearing assertion for QLever's 429. It used to mean "slow down"
    // and was retried, so one search that timed out cost up to four requests of
    // the endpoint's entire budget. It means "this query is too expensive", and
    // the answer to that is a narrower query, not another identical one.
    let http = Scripted::new(vec![(200, TAXON_CSV)]);
    http.then_always_from(1, 429, r#"{"exception":"Operation timed out"}"#);
    let request = SearchRequest::new(criteria("Gentiana lutea"), NOW);

    let error = search(&http, &request)
        .await
        .expect_err("a cancelled query is not an answer");

    assert!(
        matches!(
            error,
            SearchError::Transport {
                source: FetchError::TimedOut { .. },
                ..
            }
        ),
        "a 429 is a cancellation, not a status: {error:?}"
    );
    assert_eq!(
        http.call_count(),
        2,
        "one taxon lookup and one attempt at the rows. A retry would be a second \
         full-length query to an endpoint that has already cancelled this one."
    );
}

#[tokio::test]
async fn a_qlever_request_declares_a_time_budget_and_names_the_client() {
    // Both of these are about how the endpoint sees us. The budget is five
    // seconds under the public instance's own 30s ceiling, so a query that is
    // going to be cancelled is cancelled before it has spent the whole ceiling;
    // the identification is what makes a heavy user something an operator can
    // contact rather than an anonymous address.
    let http = Scripted::new(vec![(200, TAXON_CSV), (200, ROWS_CSV), (200, COUNTS_CSV)]);
    let request = SearchRequest::new(criteria("Gentiana lutea"), NOW);

    search(&http, &request).await.expect("the search succeeds");

    for body in http.raw_bodies() {
        assert!(
            body.contains("timeout=30s"),
            "no time budget was declared: {body}"
        );
    }
    for names in http.header_names() {
        assert!(
            names.iter().any(|name| name == "api-user-agent"),
            "the request did not say who it was: {names:?}"
        );
    }
}

#[tokio::test]
async fn a_bare_qid_skips_the_lookup_entirely() {
    let http = Scripted::new(vec![(200, ROWS_CSV), (200, COUNTS_CSV)]);
    let request = SearchRequest::new(criteria("q16521"), NOW);

    let result = search(&http, &request).await.expect("the search succeeds");

    let taxon = result.taxon.as_ref().expect("the taxon is resolved");
    assert_eq!(taxon.qid.as_deref(), Some("Q16521"));
    assert!(taxon.notes.is_empty(), "a QID needs no lookup, so no note");
    assert_eq!(http.call_count(), 2, "no lookup round trip for a QID");
}

#[tokio::test]
async fn a_lone_q_is_a_search_term_not_an_identifier() {
    // A QID has digits. Treating a bare `Q` as one would send a query the
    // endpoint cannot answer.
    assert!(!lotus_search::is_qid("Q"));
    assert!(!lotus_search::is_qid("q"));
    assert!(lotus_search::is_qid("Q1"));
    assert!(lotus_search::is_qid("q16521"));
    assert!(!lotus_search::is_qid("Q12a3"));
    assert!(!lotus_search::is_qid("Gentiana lutea"));
}

#[tokio::test]
async fn a_molfile_asked_for_as_similarity_runs_as_a_similarity_search() {
    // The mode asked for is the mode run. This used to downgrade a molfile to
    // substructure on the belief that the similarity service could not take a
    // multi-line literal; measured against the live endpoint it accepts a CTAB
    // and answers a cutoff search, so the downgrade only hid the reader's choice.
    let http = Scripted::new(vec![(200, ROWS_CSV), (200, COUNTS_CSV)]);
    let request = SearchRequest::new(
        SearchCriteria {
            structure: MOLFILE.into(),
            structure_search: lotus_search::SmilesSearchType::Similarity,
            ..criteria("")
        },
        NOW,
    );

    let result = search(&http, &request).await.expect("the search succeeds");

    assert!(
        result.query.contains("sachem:similarCompoundSearch"),
        "a molfile runs as the similarity search that was asked for: {}",
        result.query
    );
    assert!(result.query.contains("sachem:cutoff"));
    assert!(!result.query.contains("sachem:scoredSubstructureSearch"));
}

#[tokio::test]
async fn a_molfile_asked_for_as_substructure_still_runs_as_one() {
    let http = Scripted::new(vec![(200, ROWS_CSV), (200, COUNTS_CSV)]);
    let request = SearchRequest::new(
        SearchCriteria {
            structure: MOLFILE.into(),
            structure_search: lotus_search::SmilesSearchType::Substructure,
            ..criteria("")
        },
        NOW,
    );

    let result = search(&http, &request).await.expect("the search succeeds");

    assert!(
        result.query.contains("sachem:scoredSubstructureSearch"),
        "a multi-line CTAB needs the scoring substructure service: {}",
        result.query
    );
    assert!(!result.query.contains("sachem:cutoff"));
}

#[tokio::test]
async fn a_smiles_asked_for_as_similarity_keeps_its_cutoff() {
    let http = Scripted::new(vec![(200, ROWS_CSV), (200, COUNTS_CSV)]);
    let request = SearchRequest::new(
        SearchCriteria {
            structure: "c1ccccc1".into(),
            structure_search: lotus_search::SmilesSearchType::Similarity,
            structure_threshold: 0.75,
            ..criteria("")
        },
        NOW,
    );

    let result = search(&http, &request).await.expect("the search succeeds");
    assert!(result.query.contains("sachem:similarCompoundSearch"));
    assert!(result.query.contains(r#"sachem:cutoff "0.75"^^xsd:double"#));
}

#[tokio::test]
async fn an_empty_taxon_means_every_taxon() {
    let http = Scripted::new(vec![(200, ROWS_CSV), (200, COUNTS_CSV)]);
    let request = SearchRequest::new(criteria(""), NOW);

    let result = search(&http, &request).await.expect("the search succeeds");

    assert!(!result.query.contains("P171*"), "nothing narrows by taxon");
    assert_eq!(http.call_count(), 2, "and there is no lookup");
}

#[tokio::test]
async fn a_wildcard_taxon_still_reaches_every_taxon() {
    let http = Scripted::new(vec![(200, ROWS_CSV), (200, COUNTS_CSV)]);
    let request = SearchRequest::new(criteria("*"), NOW);

    let result = search(&http, &request).await.expect("the search succeeds");
    assert!(
        !result.query.contains("P171*"),
        "the wildcard is not a filter"
    );
}

#[tokio::test]
async fn invalid_filters_are_rejected_before_anything_is_sent() {
    let http = search_script();
    let request = SearchRequest::new(
        SearchCriteria {
            mass_min: 400.0,
            mass_max: 100.0,
            ..criteria("Gentiana lutea")
        },
        NOW,
    );

    let err = search(&http, &request)
        .await
        .expect_err("an inverted range is invalid");

    assert!(
        matches!(err, lotus_search::SearchError::Invalid(_)),
        "got: {err}"
    );
    assert_eq!(http.call_count(), 0, "no request was made");
}

#[tokio::test]
async fn a_rejected_query_is_not_retried_against_the_fallback_endpoint() {
    // A 4xx is the query's fault. WDQS would reject it too, and asking twice
    // only doubles the load.
    let http = Scripted::new(vec![(400, "Variable ?s was not declared")]);
    let request = SearchRequest::new(criteria("Q16521"), NOW);

    let err = search(&http, &request)
        .await
        .expect_err("the query is rejected");

    assert!(err.to_string().contains("was not declared"), "got: {err}");
    assert_eq!(http.call_count(), 1, "no fallback was attempted");
    assert!(
        http.queries().iter().all(|q| q.contains("qlever")),
        "and no other endpoint was asked"
    );
}

#[tokio::test]
async fn a_gone_endpoint_is_retried_and_then_falls_back_to_wikidata() {
    // A 502 is worth one retry at the same endpoint, because it is often a
    // blip in front of the service rather than the service itself. Only after
    // that does the query go to WDQS.
    let http = Scripted::new(vec![
        (502, "<title>502 Bad Gateway</title>"),
        (502, "<title>502 Bad Gateway</title>"),
        (200, ROWS_CSV),
        (200, COUNTS_CSV),
    ]);
    let request = SearchRequest::new(criteria("Q16521"), NOW);

    let result = search(&http, &request).await.expect("the fallback answers");

    assert_eq!(result.rows.len(), 1);
    // The rows query retries QLever once and then falls back. The count query is
    // a separate request that starts over at QLever, so the sequence is not a
    // single clean walk to the fallback.
    assert_eq!(
        http.endpoints(),
        vec![
            "https://qlever.dev/api/wikidata",
            "https://qlever.dev/api/wikidata",
            "https://query.wikidata.org/sparql",
            "https://qlever.dev/api/wikidata",
        ],
        "the rows query retried then fell back; the count query is independent"
    );
}

/// A client that fails every request to `failing` and delegates the rest.
#[derive(Clone)]
struct Failing {
    inner: Scripted,
    failing_endpoint: &'static str,
}

impl Http for Failing {
    type Response = ScriptedResponse;

    async fn post(
        &self,
        endpoint: &str,
        accept: &str,
        body: String,
    ) -> Result<Self::Response, lotus_search::FetchError> {
        if endpoint == self.failing_endpoint {
            self.inner.record(endpoint, &body);
            return Err(lotus_search::FetchError::Network(
                "connection refused".into(),
            ));
        }
        self.inner.post(endpoint, accept, body).await
    }

    async fn get(
        &self,
        url: &str,
        accept: &str,
    ) -> Result<Self::Response, lotus_search::FetchError> {
        self.inner.get(url, accept).await
    }
}

#[tokio::test]
async fn a_dropped_connection_falls_back_to_wikidata() {
    // A network failure says nothing about the query, so it is worth retrying
    // and worth falling back on. A 4xx is neither, and the test above shows it
    // is not.
    let script = Scripted::new(vec![(200, ROWS_CSV), (200, COUNTS_CSV)]);
    let http = Failing {
        inner: script.clone(),
        failing_endpoint: "https://qlever.dev/api/wikidata",
    };
    let request = SearchRequest::new(criteria("Q16521"), NOW);

    let result = search(&http, &request).await.expect("WDQS answers");

    assert_eq!(result.rows.len(), 1);
    // Each query independently tries QLever twice before giving up on it, so
    // the sequence is per-query: QLever, QLever, WDQS.
    assert_eq!(
        script.endpoints(),
        vec![
            "https://qlever.dev/api/wikidata",
            "https://qlever.dev/api/wikidata",
            "https://query.wikidata.org/sparql",
            "https://qlever.dev/api/wikidata",
            "https://qlever.dev/api/wikidata",
            "https://query.wikidata.org/sparql",
        ]
    );
}

#[tokio::test]
async fn the_returned_query_is_the_one_that_was_sent() {
    // A caller downloads "the query", and shows it as provenance. If it is not
    // the query that produced the rows, both are wrong.
    let http = search_script();
    let request = SearchRequest::new(criteria("Gentiana lutea"), NOW).with_limit(25);

    let result = search(&http, &request).await.expect("the search succeeds");

    let sent = http.queries();
    let rows_query = sent[1].split_once('|').map_or("", |(_, q)| q);
    assert!(
        rows_query.contains("LIMIT 25"),
        "the limit reached the endpoint"
    );
    assert!(
        result.query.contains("SELECT"),
        "the returned query is a base query, and the rows query is that plus a limit"
    );
    assert!(
        !result.query.contains("LIMIT"),
        "so the download is not capped"
    );
}

/// A fetched body comes back whole, and a refused one comes back as an error.
///
/// `fetch_url` is the export path's own HTTP call, and both of its failure modes are
/// silent if untested: returning an empty body on success writes an empty export that
/// reports itself as a success, and returning the body on a 404 hands the download an
/// error page. Mutation testing could make the function do either and the suite
/// noticed neither, because no test called it.
///
/// Here rather than in the module because the crate denies `expect_used`,
/// `unwrap_used` and `panic` workspace-wide, and this crate's in-module tests hold to
/// that. The script-driven tests live in this file for the same reason.
#[tokio::test]
async fn a_fetched_body_comes_back_whole() {
    let http = Scripted::new(vec![(200, "id,name\n1,aspirin\n")]);
    let body = lotus_search::fetch_url(
        &http,
        "https://example.invalid/x",
        lotus_search::ResponseFormat::Csv,
    )
    .await
    .expect("a 200 is not an error");

    assert_eq!(
        String::from_utf8_lossy(&body),
        "id,name\n1,aspirin\n",
        "the body must arrive exactly as the endpoint sent it"
    );
    assert_eq!(
        http.call_count(),
        1,
        "a prepared export URL is fetched once"
    );
}

/// A non-success status is an error carrying the endpoint's own message.
///
/// The message is what a reader sees when an export fails, so it has to be the
/// endpoint's explanation rather than a bare status code or a blank.
#[tokio::test]
async fn a_refused_url_is_an_error_naming_the_reason() {
    let http = Scripted::new(vec![(404, r#"{"exception": "no such export key"}"#)]);
    let error = lotus_search::fetch_url(
        &http,
        "https://example.invalid/gone",
        lotus_search::ResponseFormat::Csv,
    )
    .await
    .expect_err("a 404 must not come back as a body");

    assert!(
        error.to_string().contains("no such export key"),
        "the error must carry the endpoint's own reason: {error}"
    );
}

/// A wildcard and an empty box both mean "no taxon", and neither is looked up.
///
/// The wildcard resolves to no QID because it names no Wikidata entity, which is
/// exactly why `build_base_query` reads the distinction back out of `criteria.taxon`
/// rather than off the QID. Both inputs must therefore arrive at the builder as
/// `None`, and neither may cost a round trip — a `*` sent to the endpoint as a name
/// would find nothing and report that no taxon matched.
///
/// The transport is given no replies, so any request at all fails the test.
#[tokio::test]
async fn a_wildcard_and_an_empty_box_both_resolve_to_no_taxon() {
    let http = Scripted::new(Vec::new());

    for input in ["*", "  *  ", "", "   "] {
        let resolved = lotus_search::resolve_taxon(&http, input)
            .await
            .unwrap_or_else(|e| panic!("{input:?} must resolve without a lookup: {e}"));
        assert!(
            resolved.qid.is_none(),
            "{input:?} names no entity, so it resolves to no QID"
        );
        assert_eq!(
            http.call_count(),
            0,
            "{input:?} must not reach the endpoint at all"
        );
    }
}

/// An `InChIKey` in the structure box is looked up by `InChIKey`.
///
/// An `InChIKey` is three hyphen-separated blocks -- 14, 10 and 1 -- so this is
/// the test that the version block counts. `lotus-search` used to carry its own
/// `looks_like_inchikey` that read the blocks as `(Some, Some, None)`, exactly
/// two of them, and so answered `false` for every real `InChIKey`. That made
/// this arm of `resolve_structure` unreachable, and an `InChIKey` fell through
/// to the structure service and was searched for as if it were a SMILES.
///
/// The assertion is on the *query*, not on the outcome. A wrong classifier and a
/// right answer are indistinguishable from the result alone: the structure
/// service would also have found the compound, eventually, by another route. What
/// distinguishes them is which question went out, and only the query records
/// that.
#[tokio::test]
async fn an_inchikey_in_the_structure_box_is_looked_up_by_inchikey() {
    const INCHIKEY: &str = "DBOVHQOUSDWAPQ-WTONXPSSSA-N";

    let http = Scripted::new(vec![(200, LOOKUP_CSV), (200, LOOKUP_CSV)]);

    let resolved = lotus_search::resolve_structure(&http, INCHIKEY)
        .await
        .expect("an InChIKey resolves");

    assert_eq!(
        resolved.compounds,
        vec!["Q7".to_string()],
        "the compound the key identifies is the result"
    );

    let queries = http.queries();
    assert!(
        !queries.is_empty(),
        "resolving an InChIKey asks the endpoint about it"
    );
    for query in &queries {
        assert!(
            query.contains("wdt:P235"),
            "the identifier route queries the InChIKey property, but sent: {query}"
        );
        assert!(
            query.contains(&format!("VALUES ?name {{ \"{INCHIKEY}\" }}")),
            "the key itself is the value being matched, but sent: {query}"
        );
    }
}

/// The two-block string that is *not* an `InChIKey` is not treated as one.
///
/// This is the other half of the same mistake and it is the reason the old
/// predicate could not simply have been loosened: it accepted
/// `DBOVHQOUSDWAPQ-WTONXPSSSA`, which is an `InChIKey` missing its version block,
/// and would have sent it down the identifier route.
#[tokio::test]
async fn a_two_block_string_is_not_an_inchikey() {
    let http = Scripted::new(vec![(200, LOOKUP_CSV), (200, LOOKUP_CSV)]);

    assert!(
        !lotus_search::looks_like_inchikey("DBOVHQOUSDWAPQ-WTONXPSSSA"),
        "14 and 10 without the version block is not an InChIKey"
    );

    // And with nothing to reply, a structure search is attempted instead, which
    // is the behaviour the dead arm was hiding: the string reaches the structure
    // service rather than the identifier lookup.
    let outcome = lotus_search::resolve_structure(&http, "DBOVHQOUSDWAPQ-WTONXPSSSA").await;
    assert!(
        outcome.is_err() || !http.queries().iter().all(|q| q.contains("wdt:P235")),
        "a string with no version block must not take the InChIKey route"
    );
}

// ── the streaming retry loop ─────────────────────────────────────────────────
//
// `execute_streaming` has its own copy of the retry decision that `execute` has,
// and the three tests above all reach `execute` -- through `search`, which reads
// the whole body. Nothing reached the streaming copy, so its decision was
// unverified: a transport could have been sent a rejected query twice, or a
// failing one not at all, or a query that should have fallen back to WDQS not
// falling back, and the suite would have stayed green.
//
// `search_columnar` is the caller that matters -- `execute_streaming_with_fallback`
// is how every large result set is fetched -- so this is the path where a mistake
// is expensive rather than theoretical.
//
// The distinctions are between three statuses, because the code draws three:
// `500` is retryable, `502` is retryable *and* endpoint-unavailable, and `400` is
// neither.

/// Read a `StreamAnswer` to the end, so a test can assert the body survived.
async fn drain(mut answer: lotus_search::StreamAnswer) -> String {
    let mut body = String::new();
    while let Some(chunk) = answer.chunks.next_chunk().await.expect("a scripted chunk") {
        body.push_str(&String::from_utf8_lossy(&chunk));
    }
    body
}

/// A failing endpoint is tried again on the streaming path, exactly as on the
/// buffered one.
///
/// `500` is retryable, so the second attempt happens; `MAX_ATTEMPTS` is two, so
/// there is no third. The count is what makes this a test of the loop rather than
/// of the transport.
#[tokio::test]
async fn a_failing_endpoint_is_retried_once_on_the_streaming_path() {
    let http = Scripted::new(vec![(500, "boom"), (500, "boom again")]);

    let outcome = lotus_search::execute_streaming(
        &http,
        lotus_search::Endpoint::new(lotus_search::Service::Qlever),
        "SELECT * WHERE { ?s ?p ?o }",
        lotus_search::ResponseFormat::Csv,
    )
    .await;

    assert!(
        outcome.is_err(),
        "two failures and no third attempt is an error"
    );
    assert_eq!(
        http.call_count(),
        2,
        "a retryable failure is worth exactly one more attempt"
    );
}

/// A rejected query is not retried, even on the streaming path.
///
/// A `400` is the endpoint saying it will not answer this query. Asking again
/// costs a round trip and returns the same answer, and the error the caller sees
/// is the one it would have seen immediately.
#[tokio::test]
async fn a_rejected_query_is_sent_once_and_not_retried_when_streaming() {
    let http = Scripted::new(vec![(400, "malformed query")]);

    let outcome = lotus_search::execute_streaming(
        &http,
        lotus_search::Endpoint::new(lotus_search::Service::Qlever),
        "SELECT * WHERE {",
        lotus_search::ResponseFormat::Csv,
    )
    .await;

    match outcome {
        Ok(_) => panic!("a rejected query must not produce a result"),
        Err(error) => assert_eq!(
            error.status(),
            Some(400),
            "the caller's error is the rejection itself"
        ),
    }
    assert_eq!(
        http.call_count(),
        1,
        "a query the endpoint refused must not be sent again"
    );
}

/// A retry waits before it happens.
///
/// The backoff is what keeps a failing endpoint from being asked twice in the
/// same millisecond, which is the difference between a client that backs off and
/// one that adds load exactly when the service is already in trouble. It is also
/// the reason the retry above is not instantaneous.
///
/// The assertion is a lower bound on elapsed time rather than an equality: the
/// scheduler is free to take longer, and a test that failed on a slow machine
/// would be reporting the machine.
#[tokio::test]
async fn a_retry_waits_before_it_happens() {
    let http = Scripted::new(vec![(500, "boom"), (200, LOOKUP_CSV)]);

    let started = std::time::Instant::now();
    let answer = lotus_search::execute_streaming(
        &http,
        lotus_search::Endpoint::new(lotus_search::Service::Qlever),
        "SELECT * WHERE { ?s ?p ?o }",
        lotus_search::ResponseFormat::Csv,
    )
    .await;
    let elapsed = started.elapsed();

    assert!(answer.is_ok(), "the second attempt succeeds");
    assert!(
        elapsed >= std::time::Duration::from_millis(400),
        "a retry must wait out the backoff, but it took only {elapsed:?}"
    );
}

/// An unreachable `QLever` falls back to `WDQS` on the streaming path.
///
/// `502` is both retryable and endpoint-unavailable, so it exercises the retry
/// and the fallback together: two attempts at `QLever`, then the rewritten query
/// at the second service. The assertion that the third query went to a different
/// host is what makes this a test of the fallback rather than of the retry.
#[tokio::test]
async fn an_unreachable_qlever_falls_back_to_wikidata_when_streaming() {
    let http = Scripted::new(vec![
        (502, "bad gateway"),
        (502, "bad gateway"),
        (200, LOOKUP_CSV),
    ]);

    let answer = lotus_search::execute_streaming_with_fallback(
        &http,
        "SELECT * WHERE { ?s wdt:P31 wd:Q16521 }",
        lotus_search::ResponseFormat::Csv,
    )
    .await;

    assert!(answer.is_ok(), "the fallback endpoint answers");
    assert_eq!(
        drain(answer.expect("checked")).await.trim(),
        LOOKUP_CSV.trim(),
        "the body that arrives is the one the fallback service sent"
    );

    let endpoints = http.endpoints();
    assert!(
        endpoints.len() >= 3,
        "two attempts at QLever and then the fallback, got {endpoints:?}"
    );
    assert!(
        endpoints[..2].iter().all(|url| url.contains("qlever")),
        "the first two attempts are QLever's, got {endpoints:?}"
    );
    assert!(
        endpoints[2].contains("wikidata"),
        "the third is the fallback service, got {endpoints:?}"
    );
}

/// A rejected query is not run a second time against `WDQS`.
///
/// This is the fallback guard, and it is a different decision from the retry one:
/// the query was refused as malformed, so `WDQS` would refuse it too, having
/// spent a round trip and the caller's time to arrive at the same answer.
#[tokio::test]
async fn a_rejected_query_is_not_re_run_against_the_fallback_when_streaming() {
    let http = Scripted::new(vec![(400, "malformed query")]);

    let outcome = lotus_search::execute_streaming_with_fallback(
        &http,
        "SELECT * WHERE {",
        lotus_search::ResponseFormat::Csv,
    )
    .await;

    assert!(outcome.is_err(), "the rejection stands");
    assert_eq!(
        http.call_count(),
        1,
        "a refused query must not be sent to a second service, which would refuse it too"
    );
}

/// A successful streaming answer arrives in the pieces it was sent in.
///
/// The whole reason `StreamAnswer` exists is that the payload is never assembled,
/// so a test that only checked the joined text would pass for a reader that
/// buffered everything -- which is the thing that runs out of memory.
#[tokio::test]
async fn a_streamed_body_is_read_in_pieces_and_joins_to_the_same_text() {
    let body = "compound,compoundLabel\nQ1,Quercetin\n";
    let http = Scripted::with_chunk_size(vec![(200, body)], 8);

    let answer = lotus_search::execute_streaming(
        &http,
        lotus_search::Endpoint::new(lotus_search::Service::Qlever),
        "SELECT * WHERE { ?s ?p ?o }",
        lotus_search::ResponseFormat::Csv,
    )
    .await;

    let mut answer = match answer {
        Ok(answer) => answer,
        Err(error) => panic!("the scripted response streams: {error:?}"),
    };

    // Count the chunks on the way past, so "it was read in pieces" is asserted
    // rather than inferred from the joined text matching.
    let mut pieces = 0;
    let mut joined = String::new();
    while let Some(chunk) = answer.chunks.next_chunk().await.expect("a scripted chunk") {
        pieces += 1;
        joined.push_str(&String::from_utf8_lossy(&chunk));
    }

    assert!(
        pieces > 1,
        "an 8-byte chunk size over {body:?} must take several reads, took {pieces}"
    );
    assert_eq!(
        joined, body,
        "the pieces reassemble to the body that was sent"
    );
}

/// A `StreamAnswer` describes where it came from and not what it is carrying.
///
/// The doc comment on that `Debug` impl says the body is not read, because
/// reading it to describe it would defeat the point of having streamed it. That
/// is a property of the *implementation*, and the only way to hold it is to look
/// at the output: the endpoint appears, and nothing from the body does.
///
/// A `Debug` that returned `Ok(())` prints nothing at all, which is not a
/// formatting preference -- it is the difference between a log line naming the
/// service and a log line with a hole in it.
#[tokio::test]
async fn a_stream_answer_names_its_endpoint_and_not_its_body() {
    let http = Scripted::with_chunk_size(vec![(200, LOOKUP_CSV)], 8);

    let answer = lotus_search::execute_streaming(
        &http,
        lotus_search::Endpoint::new(lotus_search::Service::Qlever),
        "SELECT * WHERE { ?s ?p ?o }",
        lotus_search::ResponseFormat::Csv,
    )
    .await;

    let answer = match answer {
        Ok(answer) => answer,
        Err(error) => panic!("the scripted response streams: {error:?}"),
    };
    let rendered = format!("{answer:?}");

    assert!(
        rendered.contains("qlever"),
        "the service that answered is the useful half of the line, got {rendered:?}"
    );
    assert!(
        !rendered.contains("Berberine"),
        "the body must not be read to describe the answer, got {rendered:?}"
    );
}
