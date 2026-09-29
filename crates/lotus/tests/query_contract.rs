// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Characterization tests: the behaviour the refactor must preserve.
//!
//! These assert the *contract* of each builder (subquery nesting, where each
//! fragment lands, which optional binds are stripped) rather than exact bytes,
//! so cosmetic reformatting is allowed but a structural change is not.

// An integration test uses one dependency of the package under test and
// inherits the rest; the lint exists to keep unused crates out of what ships.
// See `result_parsing.rs` for why the panic lints are relaxed in tests.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus::models::{ElementState, SearchCriteria, SmilesSearchType};
use lotus::queries::{
    StructureKind, classify_structure, query_all_compounds, query_compounds_by_taxon,
    query_counts_from_base, query_sachem, query_taxon_search, query_with_limit,
    query_with_server_filters,
};

/// `SELECT` opens a subquery at each of these depths.
fn subquery_depth(q: &str) -> usize {
    q.matches("SELECT").count()
}

/// The filtered query is the base query with its own trailing `}` preserved and
/// the filter fragments appended after it. This is what "the insert is at the
/// outer WHERE level" means concretely.
fn tail_after_base<'a>(query: &'a str, base: &'a str) -> &'a str {
    let base_without_final_brace = base.strip_suffix('}').expect("base ends with `}`");
    query
        .strip_prefix(base_without_final_brace)
        .map_or("", |tail| tail.strip_suffix('}').unwrap_or(tail))
}

#[test]
fn compound_query_is_three_levels_deep_with_ancestry_innermost() {
    let q = query_compounds_by_taxon("Q16521");

    assert_eq!(subquery_depth(&q), 3, "SELECT + two nested subqueries");
    assert!(q.contains("?t (wdt:P171*) wd:Q16521."));

    // The ancestry filter must precede the close of the innermost subquery,
    // otherwise QLever enriches every row before filtering.
    let ancestry = q.find("P171*").expect("ancestry present");
    let innermost_close = q[ancestry..].find('}').map(|i| ancestry + i);
    assert!(
        innermost_close.is_some_and(|i| i > ancestry),
        "ancestry sits inside the innermost WHERE"
    );
    assert!(q.contains("?c wdt:P235 ?compound_inchikey"));
    assert!(q.contains("?c p:P703 ?statement"));
    assert!(q.contains("?ref pr:P248 ?r"));
    // The label prefers the `mul` term over `en`.
    assert!(q.contains("BIND(COALESCE(?compoundLabelMul, ?compoundLabelEn) AS ?compoundLabel)"));
}

#[test]
fn all_compounds_query_differs_from_by_taxon_only_by_the_ancestry_clause() {
    let all = query_all_compounds();
    let by_taxon = query_compounds_by_taxon("Q16521");

    assert!(!all.contains("P171*"), "no ancestry filter without a taxon");
    assert_eq!(subquery_depth(&all), subquery_depth(&by_taxon));
}

#[test]
fn sachem_substructure_is_isolated_in_its_own_subquery() {
    let q = query_sachem(
        "c1ccccc1",
        SmilesSearchType::Substructure,
        0.8,
        Some("Q16521"),
    );

    assert!(q.contains("PREFIX sachem:"));
    assert!(q.contains("PREFIX idsm:"));
    assert!(q.contains("SERVICE idsm:wikidata"));
    assert!(q.contains("sachem:substructureSearch"));
    assert!(q.contains("sachem:query \"c1ccccc1\""));
    assert!(q.contains("?t (wdt:P171*) wd:Q16521 ."));
    assert_eq!(
        subquery_depth(&q),
        2,
        "Sachem pre-filter subquery, then flat enrichment (no third level)"
    );
    let sachem_only = query_sachem("c1ccccc1", SmilesSearchType::Substructure, 0.8, None);
    assert!(sachem_only.contains("OPTIONAL {\n    ?c p:P703 ?statement ."));
}

#[test]
fn sachem_similarity_uses_cutoff_and_substructure_does_not() {
    let sim = query_sachem("CCO", SmilesSearchType::Similarity, 0.75, None);
    assert!(sim.contains("sachem:similarCompoundSearch"));
    assert!(sim.contains("sachem:cutoff \"0.75\"^^xsd:double"));
    assert!(!sim.contains("scoredSubstructureSearch"));

    let sub = query_sachem("CCO", SmilesSearchType::Substructure, 0.75, None);
    assert!(sub.contains("sachem:substructureSearch"));
    assert!(!sub.contains("sachem:cutoff"));
}

#[test]
fn sachem_without_taxon_makes_the_occurrence_triple_optional() {
    let q = query_sachem("CCO", SmilesSearchType::Substructure, 0.8, None);
    assert!(
        q.contains("OPTIONAL {\n    ?c p:P703 ?statement ."),
        "compounds with no occurrence data are still returned"
    );
}

#[test]
fn multiline_structure_uses_the_scored_substructure_service() {
    let molfile = "\n\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\n    0.0000    0.0000    0.0000 C   0  0  0  0  0  0\nM  END\n";
    let q = query_sachem(molfile, SmilesSearchType::Substructure, 0.8, None);
    assert!(q.contains("sachem:scoredSubstructureSearch"));
    assert!(q.contains("sachem:topn \"-1\"^^xsd:integer"));
    assert!(q.contains("'''"), "multi-line literals use triple quotes");
}

#[test]
fn mass_filter_injects_a_required_triple_at_the_outer_where_level() {
    let base = query_compounds_by_taxon("Q16521");
    let criteria = SearchCriteria {
        mass_min: 100.0,
        mass_max: 400.0,
        ..SearchCriteria::default()
    };
    let q = query_with_server_filters(&base, &criteria);

    assert!(q.contains("FILTER(?compound_mass >= 100.000000 && ?compound_mass <= 400.000000)"));
    // The insert lands *after* the base query's own closing brace, not inside
    // the inner subquery. `filters.rs` documents the insert as landing "before
    // OPTIONAL enrichment"; it does not. Preserved as-is: moving it would
    // change the query plan.
    let tail = tail_after_base(&q, &base);
    assert!(
        tail.contains("?c wdt:P2067 ?compound_mass ."),
        "insert is at the outer level: {tail}"
    );
    assert!(tail.contains("FILTER(?compound_mass"));
}

#[test]
fn year_filter_targets_the_reference_date() {
    let base = query_compounds_by_taxon("Q16521");
    let criteria = SearchCriteria {
        year_min: 1990,
        year_max: 2010,
        ..SearchCriteria::default()
    };
    let q = query_with_server_filters(&base, &criteria);
    assert!(q.contains("FILTER(YEAR(?ref_date) >= 1990 && YEAR(?ref_date) <= 2010)"));
    assert!(tail_after_base(&q, &base).contains("?r wdt:P577 ?ref_date ."));
}

#[test]
fn formula_filter_normalizes_subscripts_and_binds_element_counts() {
    let base = query_compounds_by_taxon("Q16521");
    let criteria = SearchCriteria {
        formula_enabled: true,
        formula_exact: "C₁₇H₁₂O₇".into(),
        f_state: ElementState::Required,
        c_min: 5,
        c_max: 20,
        ..SearchCriteria::default()
    };
    let q = query_with_server_filters(&base, &criteria);

    assert!(q.contains("BIND(STR(?compound_formula_raw) AS ?_formula_raw)"));
    assert!(q.contains("REPLACE(REPLACE(STR(?compound_formula_raw)"));
    assert!(q.contains("?_formula_tokens"));
    assert!(q.contains("FILTER(?_count_c >= 5 && ?_count_c <= 20)"));
    assert!(q.contains("FILTER(?_count_f > 0)"), "required halogen");
    // The exact-formula filter compares the *normalized* string, so the
    // subscripts in the user's input must not appear in the query.
    assert!(q.contains("FILTER(?_formula_norm = \"C17H12O7\")"));
    assert!(!q.contains("C₁₇H₁₂O₇"));
}

#[test]
fn excluded_halogen_becomes_an_equality_filter() {
    let base = query_compounds_by_taxon("Q16521");
    let criteria = SearchCriteria {
        formula_enabled: true,
        br_state: ElementState::Excluded,
        ..SearchCriteria::default()
    };
    let q = query_with_server_filters(&base, &criteria);
    assert!(q.contains("FILTER(?_count_br = 0)"));
}

#[test]
fn default_criteria_leave_the_query_untouched() {
    let base = query_compounds_by_taxon("Q16521");
    assert_eq!(
        query_with_server_filters(&base, &SearchCriteria::default()),
        base
    );
}

#[test]
fn counts_query_strips_display_only_optionals() {
    let base = query_compounds_by_taxon("Q16521");
    let q = query_counts_from_base(&base);

    assert!(q.starts_with("PREFIX"), "prefixes are preserved");
    // The rdfs:label scan is the expensive part: two `FILTER(LANG(…))` passes
    // per compound, run only to derive a count.
    assert!(!q.contains("?compoundLabelMul"));
    assert!(!q.contains("FILTER(LANG("));
    assert!(!q.contains("OPTIONAL { ?r wdt:P1476 ?ref_title. }"));
    assert!(!q.contains("OPTIONAL { ?c wdt:P2067 ?compound_mass. }"));
    // The projected variable *names* survive in the middle subquery's SELECT
    // list even though the triples that bound them are gone.
    assert!(q.contains("?compoundLabel"));
    assert!(q.contains("(COUNT(DISTINCT ?compound) AS ?n_compounds)"));
    assert!(q.contains("(COUNT(DISTINCT ?taxon) AS ?n_taxa)"));
    assert!(q.contains("(COUNT(DISTINCT ?ref_qid) AS ?n_references)"));
    assert!(q.contains("COUNT(DISTINCT CONCAT("));
    // Separators must not occur inside a QID or a taxon label.
    assert!(q.contains(r#"STR(?compound), "\u001F""#));
}

#[test]
fn limit_is_appended_to_the_whole_query() {
    let q = query_with_limit(&query_compounds_by_taxon("Q16521"), 250);
    assert!(q.ends_with("\nLIMIT 250"));
    assert!(q.starts_with("PREFIX"));
}

#[test]
fn rdf_export_rewrites_select_as_construct_and_binds_the_formula() {
    let select = query_compounds_by_taxon("Q16521");
    let construct = lotus::export::ExportFormat::Rdf.prepared_query(&select);

    assert!(!construct.contains("SELECT DISTINCT"));
    assert!(construct.contains("CONSTRUCT {"));
    assert!(construct.contains("?c wdt:P235 ?compound_inchikey ."));
    assert!(construct.contains("?statement ps:P703 ?t ;"));
    assert!(construct.contains("?r wdt:P356 ?ref_doi ."));
    // `?compound_formula` is only bound by the CONSTRUCT wrapper; the SELECT
    // projects `?compound_formula_raw`. The rest of the WHERE block is carried
    // over unchanged, label lookup included.
    assert!(construct.contains("AS ?compound_formula)"));
    assert!(construct.contains("?compoundLabelMul"));
    assert_eq!(
        construct.matches("SELECT").count(),
        2,
        "the outer SELECT becomes CONSTRUCT; the two inner ones stay"
    );
}

#[test]
fn csv_and_json_export_leave_the_select_untouched() {
    let select = query_compounds_by_taxon("Q16521");
    assert_eq!(
        lotus::export::ExportFormat::Csv.prepared_query(&select),
        select
    );
    assert_eq!(
        lotus::export::ExportFormat::Json.prepared_query(&select),
        select
    );
}

#[test]
fn taxon_search_escapes_the_literal_and_matches_p225_exactly() {
    let q = query_taxon_search("Gentiana \"lutea\"\\x");
    assert!(q.contains(r#"VALUES ?taxon_name { "Gentiana \"lutea\"\\x" }"#));
    assert!(q.contains("?taxon wdt:P225 ?taxon_name ."));
    assert!(
        !q.contains("FILTER"),
        "no case-insensitive scan: an exact VALUES match"
    );
}

#[test]
fn structure_classification_covers_the_three_accepted_formats() {
    const V2000: &str = "\n\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\n    0.0000    0.0000    0.0000 C   0  0  0  0  0  0\nM  END\n";
    const V3000: &str =
        "\n\n\n  0  0  0  0  0  0            999 V3000\nM  V30 BEGIN CTAB\nM  END\n";

    assert_eq!(classify_structure("   "), StructureKind::Empty);
    assert_eq!(classify_structure("c1ccccc1"), StructureKind::Smiles);
    assert_eq!(classify_structure(V2000), StructureKind::MolfileV2000);
    assert_eq!(classify_structure(V3000), StructureKind::MolfileV3000);
    assert_eq!(StructureKind::Empty.label(), "—");
    assert_eq!(StructureKind::MolfileV3000.label(), "Molfile V3000");
}

#[test]
fn a_molfile_without_a_version_tag_reads_as_smiles() {
    // `M  END` alone is not enough: the V2000/V3000 tag is what decides.
    assert_eq!(
        classify_structure("junk\nM  END\n"),
        StructureKind::Smiles,
        "undetermined input falls through to SMILES, not to an error"
    );
}
