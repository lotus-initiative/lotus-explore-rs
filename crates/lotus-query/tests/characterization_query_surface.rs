// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Characterization tests pinning the public SPARQL builder, rewriter, and
//! row-exporter surface before `lotus-query` and `lotus-model` structural moves.

#![allow(
    unused_crate_dependencies,
    clippy::expect_used,
    reason = "integration test uses crate dependencies selectively and unwraps test fixtures"
)]

use lotus_model::{
    ColumnarResultSet, CompoundEntry, ElementState, SearchCriteria, SmilesSearchType,
    TaxonNomenclature,
};
use lotus_query::{
    ExportFormat, FallbackService, Nomenclature, RowExporter,
    all_compounds_including_untaxonomised_query, all_compounds_query,
    compounds_by_taxon_query_with, construct_from_select, counts_query, dictionary_queries,
    exact_compounds_query, limit_query, occurrences_query, structure_search_query_with,
    wdqs_fallback, with_filters,
};
use std::sync::Arc;

const YEAR: u16 = 2026;

fn sample_entry() -> CompoundEntry {
    CompoundEntry {
        compound_qid: Arc::from("Q18216"),
        name: Arc::from("aspirin"),
        inchikey: Some(Arc::from("BSYNRYMUTXBXSQ-UHFFFAOYSA-N")),
        smiles: Some(Arc::from("CC(=O)Oc1ccccc1C(=O)O")),
        mass: Some(180.0423),
        formula: Some(Arc::from("C9H8O4")),
        taxon_qid: Arc::from("Q158572"),
        taxon_name: Arc::from("Gentiana lutea"),
        reference_qid: Arc::from("Q55712842"),
        reference_node: Arc::from(""),
        ref_title: Some(Arc::from("Aspirin study")),
        ref_doi: Some(Arc::from("10.1000/test")),
        pub_year: Some(2020),
        statement: Some(Arc::from("S1")),
    }
}

#[test]
fn six_base_query_shapes_match_pinned_contracts() {
    let nom = Nomenclature::from(&TaxonNomenclature::ALL_ON);

    let untax = all_compounds_including_untaxonomised_query();
    let all_req = all_compounds_query();
    let by_taxon = compounds_by_taxon_query_with("Q158572", &nom);
    let exact_no_taxon = exact_compounds_query(&["Q18216".to_string()], None);
    let exact_with_taxon = exact_compounds_query(&["Q18216".to_string()], Some(("Q158572", &nom)));
    let struct_no_taxon =
        structure_search_query_with("CCO", SmilesSearchType::Substructure, 0.8, None, &nom);
    let struct_with_taxon = structure_search_query_with(
        "CCO",
        SmilesSearchType::Similarity,
        0.75,
        Some("Q158572"),
        &nom,
    );

    assert!(
        untax.contains("OPTIONAL {\n        {\n          SELECT *")
            && untax.contains("p:P703 ?statement_id ."),
        "untaxonomised base shape must make P703 OPTIONAL: {untax}"
    );
    assert!(
        all_req.contains("p:P703 ?statement_id .")
            && !all_req.contains("OPTIONAL {\n        {\n          SELECT *"),
        "all_compounds base shape must require P703: {all_req}"
    );
    assert!(
        by_taxon.contains("wd:Q158572") && by_taxon.contains("wdt:P171*"),
        "taxon base shape must traverse P171* from Q158572: {by_taxon}"
    );
    assert!(
        exact_no_taxon.contains("VALUES ?c { wd:Q18216 }")
            && exact_no_taxon.contains("OPTIONAL {\n            { SELECT * WHERE {"),
        "exact query without taxon must make P703 OPTIONAL: {exact_no_taxon}"
    );
    assert!(
        exact_with_taxon.contains("VALUES ?c { wd:Q18216 }")
            && !exact_with_taxon.contains("OPTIONAL {\n            { SELECT * WHERE {")
            && exact_with_taxon.contains("wd:Q158572"),
        "exact query with taxon must require P703 and bind Q158572: {exact_with_taxon}"
    );
    assert!(
        struct_no_taxon.contains("SERVICE idsm:wikidata")
            && struct_no_taxon.contains("sachem:substructureSearch"),
        "structure query must target IDSM substructure service: {struct_no_taxon}"
    );
    assert!(
        struct_with_taxon.contains("sachem:similarCompoundSearch")
            && struct_with_taxon.contains("0.75")
            && struct_with_taxon.contains("wd:Q158572"),
        "similarity structure query with taxon must include threshold and taxon QID: {struct_with_taxon}"
    );
}

#[test]
fn query_rewriters_preserve_exact_semantics() {
    let mut criteria = SearchCriteria {
        taxon: "Q158572".into(),
        reference: "Q55712842".into(),
        mass_min: 100.0,
        mass_max: 500.0,
        year_min: 2000,
        year_max: 2024,
        formula_enabled: true,
        formula_exact: "C₉H₈O₄".into(),
        f_state: ElementState::Excluded,
        ..SearchCriteria::up_to_year(YEAR)
    };
    criteria.c_min = 5;

    let base = compounds_by_taxon_query_with("Q158572", &Nomenclature::from(&criteria));
    let filtered = with_filters(&base, &criteria, YEAR);

    assert!(
        filtered.contains("VALUES ?r { wd:Q55712842 }"),
        "with_filters must inject reference VALUES: {filtered}"
    );
    assert!(
        filtered.contains("FILTER(?compound_mass >= 100.000000 && ?compound_mass <= 500.000000)"),
        "with_filters must inject mass range: {filtered}"
    );
    assert!(
        filtered.contains("YEAR(?ref_date) >= 2000 && YEAR(?ref_date) <= 2024"),
        "with_filters must inject year range: {filtered}"
    );
    assert!(
        filtered.contains("FILTER(?_formula_norm = \"C9H8O4\")"),
        "with_filters must subscript-normalize exact formula: {filtered}"
    );
    assert!(
        filtered.contains("FILTER(?_count_f = 0)"),
        "with_filters must exclude F: {filtered}"
    );

    let occ = occurrences_query(&filtered);
    assert!(
        occ.contains("(xsd:integer(STRAFTER(STR(?c), \"Q\")) AS ?compound)")
            && occ.contains("(xsd:integer(STRAFTER(STR(?t), \"Q\")) AS ?taxon)")
            && occ.contains("?taxon_name")
            && occ.contains("(xsd:integer(STRAFTER(STR(?r), \"Q\")) AS ?ref_qid)")
            && !occ.contains("?compoundLabel"),
        "occurrences_query must project 4 columns without ?compoundLabel: {occ}"
    );
    assert!(
        occ.contains("?r wdt:P577 ?ref_date ."),
        "occurrences_query must re-add ?ref_date binding when year filter is active: {occ}"
    );

    let counts = counts_query(&filtered);
    assert!(
        counts.contains("COUNT(DISTINCT ?compound) AS ?n_compounds"),
        "counts_query must project aggregate counters: {counts}"
    );
    assert!(
        counts.contains("?r wdt:P577 ?ref_date ."),
        "counts_query must preserve ?ref_date binding when year filter is active: {counts}"
    );

    let limited = limit_query(&base, 25);
    assert!(
        limited.ends_with("\nLIMIT 25"),
        "limit_query must append LIMIT: {limited}"
    );

    let construct = construct_from_select(&limited);
    assert!(
        construct.contains("CONSTRUCT {") && construct.trim_end().ends_with("LIMIT 25"),
        "construct_from_select must build CONSTRUCT and keep trailing LIMIT: {construct}"
    );

    let (svc, rewritten) = wdqs_fallback(&base);
    assert_eq!(
        svc,
        FallbackService::Main,
        "full base query routes to WDQS Main with scholarly SERVICE"
    );
    assert!(
        rewritten.contains("SERVICE <https://query-scholarly-next.wikidata.org/sparql>"),
        "wdqs_fallback must wrap reference metadata in scholarly SERVICE: {rewritten}"
    );

    let dict = dictionary_queries(&[18216], &[55_712_842]);
    assert_eq!(
        dict.len(),
        1,
        "small combined dictionary batch must coalesce into a single query"
    );
}

#[test]
fn row_exporter_formats_are_stable() {
    let set = ColumnarResultSet::from_entries(&[sample_entry()]);

    let render = |fmt: ExportFormat| {
        let mut out = String::new();
        RowExporter::new(fmt, &set).for_each_chunk(|c| out.push_str(c));
        out
    };

    let csv = render(ExportFormat::Csv);
    assert_eq!(
        csv,
        "compound,compoundLabel,compound_inchikey,compound_smiles,compound_mass,compound_formula,taxon,taxon_name,ref,ref_title,ref_doi,ref_year,statement\n\
         Q18216,aspirin,BSYNRYMUTXBXSQ-UHFFFAOYSA-N,CC(=O)Oc1ccccc1C(=O)O,180.0423,C9H8O4,Q158572,Gentiana lutea,Q55712842,Aspirin study,10.1000/TEST,2020,S1\n",
        "CSV export bytes changed"
    );

    let json_raw = render(ExportFormat::Json);
    let json: serde_json::Value = serde_json::from_str(&json_raw).expect("valid SPARQL JSON");
    assert_eq!(
        json.pointer("/results/bindings/0/compound/value"),
        Some(&serde_json::json!("Q18216")),
        "SPARQL JSON compound value changed"
    );
    assert_eq!(
        json.pointer("/results/bindings/0/ref_year/value"),
        Some(&serde_json::json!("2020")),
        "SPARQL JSON ref_year changed"
    );

    let ttl = render(ExportFormat::Rdf);
    assert!(
        ttl.contains("wd:Q18216 rdfs:label \"aspirin\" .")
            && ttl.contains("wd:Q18216 wdt:P235 \"BSYNRYMUTXBXSQ-UHFFFAOYSA-N\" .")
            && ttl.contains("<http://www.wikidata.org/entity/statement/S1> ps:P703 wd:Q158572 ."),
        "Turtle export triples changed: {ttl}"
    );
}
