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
use lotus_search::{Http, SearchRequest, search};

const NOW: u16 = 2026;
const MOLFILE: &str = "\n\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\n    0.0000    0.0000    0.0000 C   0  0  0  0  0  0\nM  END\n";

const TAXON_CSV: &str = "taxon,taxon_name\nhttp://www.wikidata.org/entity/Q16521,Gentiana lutea\n";

const ROWS_CSV: &str = "compound,compoundLabel,compound_inchikey,compound_smiles_conn,compound_smiles_iso,compound_mass,compound_formula,taxon,taxon_name,ref_qid,ref_title,ref_doi,ref_date,statement\nQ1,Quercetin,IK1,CCO,,180.16,C9H8O4,Q16521,Gentiana lutea,Q100,T,10.1/a,2021,\n";

const COUNTS_CSV: &str = "n_entries,n_entries_unique,n_compounds,n_taxa,n_references\n1,1,1,1,1\n";

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
    // rate-limited. Reporting no totals would be worse than reporting the ones
    // the rows themselves support.
    let http = Scripted::new(vec![(200, TAXON_CSV), (200, ROWS_CSV)]);
    http.then_always_from(2, 429, "slow down");
    let request = SearchRequest::new(criteria("Gentiana lutea"), NOW);

    let result = search(&http, &request).await.expect("the rows are usable");

    assert_eq!(result.rows.len(), 1);
    let stats = result.stats.expect("a local count is substituted");
    assert_eq!(stats.n_entries, 1);
    assert_eq!(stats.n_compounds, 1);
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
async fn a_molfile_asked_for_as_similarity_is_run_as_a_substructure_search() {
    // The similarity service cannot take a multi-line literal, so a molfile
    // with a similarity threshold is downgraded. The web app had this guard and
    // the API did not, which is why it lives here now.
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
        result.query.contains("sachem:scoredSubstructureSearch"),
        "a molfile runs as a substructure search"
    );
    assert!(!result.query.contains("sachem:similarCompoundSearch"));
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
