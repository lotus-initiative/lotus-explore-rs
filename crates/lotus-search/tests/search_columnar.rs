// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Searching the whole result set, streamed, with exact counts.
//!
//! The properties these pin are the three that separate this path from
//! [`lotus_search::search`]: no `LIMIT` in the query, no second `COUNT` query,
//! and a body that is never in memory all at once.

// The panic lints keep library code free of panics on external input. A test
// that fails on a bad fixture is reporting, not panicking.
#![allow(unused_crate_dependencies, clippy::expect_used)]

use lotus_model::SearchCriteria;
use lotus_search::testing::Scripted;
use lotus_search::{SearchRequest, search_columnar};

const HEADER: &str = "compound,compoundLabel,compound_inchikey,compound_mass,compound_formula,taxon,taxon_name,ref_qid,ref_node,statement_id\n";

fn payload() -> String {
    format!(
        "{HEADER}\
         Q1,first,,120.5,C2H6O,Q10,Homo,Q100,,http://www.wikidata.org/entity/statement/Q1-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE\n\
         Q2,\"second, compound\",,300,C6H12O6,Q10,Homo,Q100,\n\
         Q1,first,,120.5,C2H6O,Q10,Homo,Q100,\n\
         Q3,third,,,,\n\
         Q4,fourth,,50,,Q11,Plant,Q200,\n"
    )
}

/// A transport scripted with the results query answered by `body`.
///
/// One reply, because an empty taxon input means "every taxon" and is answered
/// without a round trip. That is what makes `call_count` below a real assertion:
/// a count query would need a second one.
fn scripted(body: &str, chunk_size: usize) -> Scripted {
    Scripted::with_chunk_size(vec![(200, body)], chunk_size)
}

#[test]
fn the_query_carries_no_limit() {
    // The old path appended `LIMIT 500` server-side, which is why its filters
    // could only ever see 500 rows: the truncation happened before anything
    // downstream could undo it.
    let http = scripted(&payload(), 0);
    let result = block_on(search_columnar(
        &http,
        &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
    ))
    .expect("the scripted transport answers");

    assert!(
        !result.query.contains("LIMIT"),
        "a limit would hide rows the filters have to see: {}",
        result.query
    );
}

#[test]
fn the_whole_set_arrives_and_its_counts_are_exact() {
    let http = scripted(&payload(), 0);
    let result = block_on(search_columnar(
        &http,
        &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
    ))
    .expect("the scripted transport answers");
    let stats = result.set.stats();

    assert_eq!(result.set.row_count(), 5, "every row, duplicates included");
    assert_eq!(stats.n_entries, 5);
    assert_eq!(
        stats.n_entries_unique, 4,
        "rows one and three are the same triple"
    );
    assert_eq!(stats.n_compounds, 4);
    assert_eq!(stats.n_taxa, 2);
    assert_eq!(stats.n_references, 2);
}

#[test]
fn no_count_query_is_sent() {
    // The set holds every row and counts them itself, so the second query is
    // pure cost -- and it was the query whose two deleted blocks could silently
    // make a filter count nothing.
    let http = scripted(&payload(), 0);
    block_on(search_columnar(
        &http,
        &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
    ))
    .expect("the scripted transport answers");

    assert_eq!(
        http.call_count(),
        1,
        "the results query, and nothing else: {:?}",
        http.queries()
    );
    assert!(
        !http.queries().iter().any(|q| q.contains("COUNT")),
        "no query should ask the endpoint to count: {:?}",
        http.queries()
    );
}

#[test]
fn the_result_is_the_same_however_the_body_arrives() {
    let whole = block_on(search_columnar(
        &scripted(&payload(), 0),
        &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
    ))
    .expect("the scripted transport answers");

    // Every chunk size from one byte up, so the reader has to carry a record, a
    // quoted field and a multi-byte character across a boundary.
    for size in 1..=payload().len() {
        let streamed = block_on(search_columnar(
            &scripted(&payload(), size),
            &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
        ))
        .expect("the scripted transport answers");

        assert_eq!(
            streamed.set.stats(),
            whole.set.stats(),
            "chunk size {size} changed the counts"
        );
        for row in 0..whole.set.row_count() {
            assert_eq!(
                streamed.set.entry(row),
                whole.set.entry(row),
                "chunk size {size} changed row {row}"
            );
        }
    }
}

#[test]
fn a_statement_comes_back_rebuilt_from_the_compound_and_its_uuid() {
    let http = scripted(&payload(), 3);
    let result = block_on(search_columnar(
        &http,
        &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
    ))
    .expect("the scripted transport answers");

    assert_eq!(
        result.set.statement_text(0).as_deref(),
        Some("Q1-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE"),
        "the 41-byte URI prefix and the compound prefix are both re-derived"
    );
    assert_eq!(result.set.statement_text(1), None, "no statement, no cell");
}

#[test]
fn a_row_with_no_taxon_does_not_count_as_a_taxon() {
    let http = scripted(&payload(), 0);
    let result = block_on(search_columnar(
        &http,
        &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
    ))
    .expect("the scripted transport answers");

    assert_eq!(
        result.set.taxon_qid(3),
        None,
        "row three is the one with no taxon"
    );
    assert_eq!(
        result.set.stats().n_taxa,
        2,
        "the two real taxa, and not the absent one"
    );
}

#[test]
fn normalized_occurrence_stream_enriches_compound_and_reference_dictionaries() {
    let occurrences_csv = "\
compound,taxon,taxon_name,ref_qid
1,10,Homo,100
2,10,Homo,100
1,10,Homo,100
3,,,
4,11,Plant,200
";
    let entities_csv = "\
compound,compoundLabel,compound_inchikey,compound_smiles_iso,compound_mass,compound_formula,ref_qid,ref_title,ref_doi,ref_year
1,first,KEY1-A-B,CCO,120.5,C2H6O,,,,
2,\"second, compound\",KEY2-C-D,C(C)O,300,C6H12O6,,,,
3,third,,,,,,,,
4,fourth,,,50,,,,,
999,unreferenced_compound,EXTRA-KEY,C,12,CH4,,,,
,,,,,,100,Paper One,10.1000/one,2020
,,,,,,200,Paper Two,https://doi.org/10.1000/TWO,2022
,,,,,,888,Unreferenced Paper,10.1000/ignored,1999
";

    let http = Scripted::with_chunk_size(vec![(200, occurrences_csv), (200, entities_csv)], 19);
    let result = block_on(search_columnar(
        &http,
        &SearchRequest::new(SearchCriteria::up_to_year(2025), 2025),
    ))
    .expect("two-stream normalized pipeline succeeds");

    assert_eq!(http.call_count(), 2);
    let queries = http.queries();
    assert_eq!(
        queries.get(1).map(String::as_str),
        Some(
            format!(
                "{}|{}&timeout=30s",
                lotus_search::QLEVER_WIKIDATA,
                lotus_query::entity_dictionary_query(&[1, 2, 3, 4], &[100, 200])
            )
            .as_str()
        ),
        "combined dictionary query requests both compound and reference QIDs in one request"
    );

    let set = &result.set;
    let stats = set.stats();
    assert_eq!(stats.n_entries, 5);
    assert_eq!(stats.n_entries_unique, 4);
    assert_eq!(
        stats.n_compounds, 4,
        "extra dictionary rows not present in occurrences must not inflate n_compounds"
    );
    assert_eq!(stats.n_taxa, 2);
    assert_eq!(
        stats.n_references, 2,
        "extra dictionary rows not present in occurrences must not inflate n_references"
    );

    assert_eq!(set.compound_label(0), Some("first"));
    assert_eq!(set.inchikey(0), Some("KEY1-A-B"));
    assert_eq!(set.smiles(0), Some("CCO"));
    assert_eq!(set.mass(0), Some(120.5));
    assert_eq!(set.formula(0), Some("C2H6O"));
    assert_eq!(set.taxon_label(0), Some("Homo"));
    assert_eq!(set.reference_title(0), Some("Paper One"));
    assert_eq!(set.reference_doi(0), Some("10.1000/ONE"));
    assert_eq!(set.pub_year(0), Some(2020));

    assert_eq!(set.compound_label(1), Some("second, compound"));
    assert_eq!(set.reference_doi(4), Some("10.1000/TWO"));
    assert_eq!(set.pub_year(4), Some(2022));

    let by_ref = set.plan_filter(&lotus_model::FilterSpec {
        reference: "Paper Two".to_string(),
        ..Default::default()
    });
    assert_eq!(by_ref.surviving_rows(set), vec![4]);
}

#[test]
fn large_dictionary_queries_switch_from_values_to_lotus_subquery() {
    let qids: Vec<u32> = (1..=50_001).collect();
    let compounds = lotus_query::compound_dictionary_query(&qids);
    assert!(
        !compounds.contains("VALUES"),
        "over 50,000 QIDs must use the parameter-free subquery to stay under HTTP body limits"
    );
    assert!(
        compounds.contains("SELECT DISTINCT ?c ?compound_inchikey ?compound_smiles_conn"),
        "compound dictionary scan must use a single-pass distinct subquery: {compounds}"
    );

    let references = lotus_query::reference_dictionary_query(&qids);
    assert!(!references.contains("VALUES"));
    assert!(references.contains("SELECT DISTINCT ?r"));
    assert!(
        references.contains("SELECT ?_s"),
        "reference dictionary fallback must materialize LOTUS statements before joining prov:wasDerivedFrom: {references}"
    );
    assert!(
        !references.contains("p:P703/prov:wasDerivedFrom/pr:P248"),
        "unconstrained 3-hop sequence path over 1.33B prov:wasDerivedFrom triples must never be emitted: {references}"
    );
}

#[test]
fn seeded_reader_skips_already_populated_entities_during_enrichment() {
    let previous = lotus_model::ColumnarResultSet::from_entries(&[lotus_model::CompoundEntry {
        compound_qid: std::sync::Arc::from("Q1"),
        name: std::sync::Arc::from("Seeded One"),
        inchikey: Some(std::sync::Arc::from("SEED-KEY-1")),
        smiles: Some(std::sync::Arc::from("C")),
        mass: Some(16.04),
        formula: Some(std::sync::Arc::from("CH4")),
        taxon_qid: std::sync::Arc::from("Q10"),
        taxon_name: std::sync::Arc::from("Homo"),
        reference_qid: std::sync::Arc::from("Q100"),
        reference_node: std::sync::Arc::from(""),
        ref_title: Some(std::sync::Arc::from("Seeded Paper")),
        ref_doi: Some(std::sync::Arc::from("10.1000/SEED")),
        pub_year: Some(2021),
        statement: None,
    }]);

    let mut reader = lotus_query::CsvColumnarReader::new();
    reader
        .feed(b"compound,taxon,taxon_name,ref_qid\n1,10,Homo,100\n2,11,Plant,200\n")
        .expect("feed occurrences");
    reader.seed_from(&previous);

    assert_eq!(
        reader.compound_qids().collect::<Vec<_>>(),
        vec![2],
        "Q1 was seeded from previous set, so only Q2 needs enrichment"
    );
    assert_eq!(
        reader.reference_qids().collect::<Vec<_>>(),
        vec![200],
        "Q100 was seeded from previous set, so only Q200 needs enrichment"
    );
}

#[test]
fn extra_entities_from_unconstrained_dictionary_stream_seed_next_search() {
    let mut first = lotus_query::CsvColumnarReader::new();
    first
        .feed(b"compound,taxon,taxon_name,ref_qid\n1,10,Plantae,100\n")
        .expect("feed first occurrences");
    first.next_enrichment_stream().expect("switch to enrich");
    first
        .feed(
            b"compound,compoundLabel,compound_inchikey,compound_smiles_iso,compound_mass,compound_formula,ref_qid,ref_title,ref_doi,ref_year\n\
              1,Plant Compound,KEY-1,C,16,CH4,,,,\n\
              2,Fungi Compound,KEY-2,CC,30,C2H6,,,,\n\
              ,,,,,,100,Plant Paper,10.1000/p1,2020\n\
              ,,,,,,200,Fungi Paper,10.1000/f1,2022\n",
        )
        .expect("feed global dictionary");
    let extra = first
        .take_extra_entities()
        .expect("non-occurring Fungi entities captured in extra set");
    let first_set = first.finish().expect("finish first set");
    assert_eq!(first_set.stats().n_compounds, 1);

    let mut second = lotus_query::CsvColumnarReader::new();
    second
        .feed(b"compound,taxon,taxon_name,ref_qid\n2,20,Fungi,200\n")
        .expect("feed second occurrences");
    second.seed_from(&first_set);
    second.seed_from(&extra);

    assert!(
        second.compound_qids().next().is_none() && second.reference_qids().next().is_none(),
        "extra entities captured during first unconstrained dictionary stream must seed 100% of second search"
    );
    let second_set = second.finish().expect("finish second set");
    assert_eq!(second_set.compound_label(0), Some("Fungi Compound"));
    assert_eq!(second_set.reference_title(0), Some("Fungi Paper"));
}

#[test]
fn occurrences_query_rebinds_ref_date_when_year_filter_is_active() {
    let mut criteria = SearchCriteria::up_to_year(2025);
    criteria.taxon = "Q756".to_string();
    criteria.year_min = 2015;
    criteria.year_max = 2020;
    let full = lotus_query::with_filters(
        &lotus_query::compounds_by_taxon_query("Q756"),
        &criteria,
        2025,
    );
    let occ = lotus_query::occurrences_query(&full);
    assert!(
        !occ.contains("?compoundLabel"),
        "display compound properties must be stripped from the occurrence query"
    );
    assert!(
        !occ.contains("?ref_title"),
        "display reference metadata must be stripped from the occurrence query"
    );
    assert!(
        occ.contains("?r wdt:P577 ?ref_date .\nFILTER(BOUND(?ref_date)"),
        "stripped ?ref_date must be rebound before the year filter: {occ}"
    );
}

/// Drive a future to completion on the current thread.
///
/// The search is a single async fn with no reactor requirement beyond the
/// transport's, and the test double has no timers, so a hand-rolled block-on is
/// enough -- and it keeps the test from needing an executor this crate does not
/// already depend on for wasm.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Waker};

    let mut context = Context::from_waker(Waker::noop());
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::hint::spin_loop(),
        }
    }
}
