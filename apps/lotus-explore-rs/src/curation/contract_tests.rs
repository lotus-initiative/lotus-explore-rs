// SPDX-License-Identifier: AGPL-3.0-only
//! Characterization tests for the curation logic the refactor must preserve:
//! TSV input parsing, row identity, and `QuickStatements` bundle assembly.
//!
//! Everything here is pure. The parts that reach the network (structure
//! conversion, taxon and reference lookups) are out of scope: what they must
//! produce, not how they get there.

#![allow(clippy::expect_used)]
#![allow(clippy::indexing_slicing)]

use super::{CurationInputRow, CurationResultRow, CurationStatus, QuickStatementsBundle};
use crate::curation::{
    build_quickstatements_bundle, example_rows, parse_tsv_rows, row_uniqueness_key,
};

fn input(name: &str, smiles: &str, taxon: Option<&str>, doi: Option<&str>) -> CurationInputRow {
    CurationInputRow {
        name: name.into(),
        smiles: smiles.into(),
        taxon: taxon.map(Into::into),
        doi: doi.map(Into::into),
    }
}

fn result(
    input: CurationInputRow,
    status: CurationStatus,
    deps: &[&str],
    qs: &[&str],
) -> CurationResultRow {
    CurationResultRow {
        input,
        canonical_smiles: None,
        inchikey: None,
        inchi: None,
        formula: None,
        exact_mass: None,
        mass_warning: None,
        wikidata_qid: None,
        status,
        note: String::new(),
        dependency_blocks: deps.iter().map(|s| (*s).to_string()).collect(),
        quickstatements: qs.iter().map(|s| (*s).to_string()).collect(),
    }
}

// ── TSV input ────────────────────────────────────────────────────────────────

#[test]
fn tsv_columns_are_matched_by_name_not_position() {
    let rows = parse_tsv_rows("smiles\tname\ttaxon\nCCO\tEthanol\tOenothera\n").expect("valid TSV");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Ethanol");
    assert_eq!(rows[0].smiles, "CCO");
}

#[test]
fn organism_is_accepted_as_a_synonym_for_taxon() {
    let rows = parse_tsv_rows("name\tsmiles\torganism\nA\tCCO\tOenothera\n").expect("valid TSV");
    assert_eq!(rows[0].taxon.as_deref(), Some("Oenothera"));
}

#[test]
fn a_missing_name_or_smiles_column_is_rejected_but_optional_columns_are_not() {
    let err = parse_tsv_rows("smiles\ttaxon\nCCO\tOenothera\n").expect_err("no name column");
    assert!(err.to_string().contains("name"), "got: {err}");

    let rows = parse_tsv_rows("name\tsmiles\nA\tCCO\n").expect("valid TSV");
    assert_eq!(rows[0].taxon, None);
    assert_eq!(rows[0].doi, None);
}

#[test]
fn rows_without_a_name_or_a_smiles_are_skipped_not_rejected() {
    let rows =
        parse_tsv_rows("name\tsmiles\ttaxon\nA\tCCO\tX\nB\t\tX\nC\tCCN\tX\n").expect("valid TSV");
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["A", "C"], "an empty SMILES cell drops the row");
}

#[test]
fn a_row_whose_first_cell_is_empty_shifts_every_column_left() {
    // Known bug, pinned so that fixing it is a deliberate change. `parse_tsv_rows`
    // trims each line before splitting it on tabs, and `str::trim` eats the
    // leading tab — so the row `\tCCO\tX` is read as name="CCO", smiles="X".
    // The row is then kept with the wrong name and a SMILES that is really a
    // taxon. A real export has a name in column one, so this is rare, but the
    // fix is to split before trimming.
    let rows = parse_tsv_rows("name\tsmiles\ttaxon\n\tCCO\tX\n").expect("valid TSV");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "CCO", "the SMILES landed in the name column");
    assert_eq!(rows[0].smiles, "X", "the taxon landed in the SMILES column");
}

#[test]
fn a_trailing_empty_cell_also_survives_because_only_the_line_ends_are_trimmed() {
    // The mirror case does *not* misparse: `trim` removes trailing tabs, but the
    // remaining empty cells are still present, so the columns stay aligned.
    let rows = parse_tsv_rows("name\tsmiles\ttaxon\tdoi\nA\tCCO\tX\t\n").expect("valid TSV");
    assert_eq!(rows[0].name, "A");
    assert_eq!(rows[0].smiles, "CCO");
    assert_eq!(rows[0].taxon.as_deref(), Some("X"));
    assert_eq!(rows[0].doi, None);
}

#[test]
fn extra_columns_past_the_last_interesting_one_are_ignored() {
    // A real export carries a dozen more fields; none may shift the ones we read.
    let rows =
        parse_tsv_rows("name\tsmiles\ttaxon\tdoi\textra1\textra2\nA\tCCO\tX\t10.1/a\tfoo\tbar\n")
            .expect("valid TSV");
    assert_eq!(rows[0].name, "A");
    assert_eq!(rows[0].doi.as_deref(), Some("10.1/A"));
}

#[test]
fn header_only_and_empty_input_yield_no_rows() {
    assert!(
        parse_tsv_rows("name\tsmiles\n")
            .expect("valid TSV")
            .is_empty()
    );
    assert!(parse_tsv_rows("").expect("empty is valid").is_empty());
}

#[test]
fn blank_lines_and_crlf_are_tolerated() {
    let rows = parse_tsv_rows("name\tsmiles\r\n\r\nA\tCCO\r\n\r\nB\tCCN\r\n").expect("valid TSV");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].smiles, "CCN");
}

#[test]
fn the_example_rows_are_well_formed() {
    // The example set is what a first-time user sees; it must survive the
    // parser, taxon and DOI included.
    let rows = example_rows();
    assert_eq!(rows.len(), 3);
    for r in &rows {
        assert!(!r.name.trim().is_empty(), "example name: {:?}", r.name);
        assert!(!r.smiles.trim().is_empty(), "example smiles: {:?}", r.name);
        assert!(r.taxon.is_some(), "example taxon: {:?}", r.name);
        assert!(r.doi.is_some(), "example doi: {:?}", r.name);
    }
}

// ── Row identity ─────────────────────────────────────────────────────────────

#[test]
fn row_identity_is_smiles_taxon_doi_with_taxon_and_doi_normalised() {
    let a = input(
        "A",
        "CCO",
        Some("Voacanga africana"),
        Some("https://doi.org/10.1000/abc"),
    );
    let b = input(
        "different name",
        " CCO ",
        Some("voacanga AFRICANA"),
        Some("10.1000/ABC"),
    );
    assert_eq!(
        row_uniqueness_key(&a),
        row_uniqueness_key(&b),
        "name is not part of the identity: the same finding is the same row"
    );
    assert_eq!(
        row_uniqueness_key(&a),
        "CCO\tvoacanga africana\t10.1000/ABC"
    );
}

#[test]
fn an_absent_taxon_or_doi_contributes_an_empty_field() {
    assert_eq!(
        row_uniqueness_key(&input("A", "CCO", None, None)),
        "CCO\t\t"
    );
    assert_eq!(
        row_uniqueness_key(&input("A", "CCO", Some("   "), Some("  "))),
        "CCO\t\t",
        "whitespace is not a value"
    );
}

#[test]
fn two_rows_differing_only_in_smiles_are_distinct() {
    assert_ne!(
        row_uniqueness_key(&input("A", "CCO", Some("X"), None)),
        row_uniqueness_key(&input("A", "CCN", Some("X"), None))
    );
}

// ── QuickStatements bundle ───────────────────────────────────────────────────

#[test]
fn dependencies_are_deduplicated_preserving_first_appearance() {
    let rows = vec![
        result(
            input("A", "CCO", None, None),
            CurationStatus::PendingDependencies,
            &["DEP-2", "DEP-1"],
            &["A1"],
        ),
        result(
            input("B", "CCN", None, None),
            CurationStatus::PendingDependencies,
            &["DEP-1", "DEP-3"],
            &["B1"],
        ),
    ];
    let bundle = build_quickstatements_bundle(&rows);
    assert_eq!(bundle.dependencies.as_ref(), "DEP-2\n\nDEP-1\n\nDEP-3");
}

#[test]
fn a_dependency_block_that_is_a_prefix_of_another_is_not_deduplicated() {
    // A taxon's `CREATE` must not be swallowed by a compound's.
    let rows = vec![
        result(
            input("A", "CCO", None, None),
            CurationStatus::PendingDependencies,
            &["CREATE"],
            &[],
        ),
        result(
            input("B", "CCN", None, None),
            CurationStatus::PendingDependencies,
            &["CREATE\nLAST|P31|Q16521"],
            &[],
        ),
    ];
    let bundle = build_quickstatements_bundle(&rows);
    assert!(
        bundle
            .dependencies
            .as_ref()
            .contains("CREATE\nLAST|P31|Q16521")
    );
}

#[test]
fn main_statements_join_within_a_row_and_blank_line_between_rows() {
    let rows = vec![
        result(
            input("A", "CCO", None, None),
            CurationStatus::NewCompound,
            &[],
            &["CREATE", "LAST|P31|Q11173"],
        ),
        result(
            input("B", "CCN", None, None),
            CurationStatus::NewCompound,
            &[],
            &["CREATE", "LAST|P31|Q11173"],
        ),
    ];
    let bundle = build_quickstatements_bundle(&rows);
    assert_eq!(
        bundle.main.as_ref(),
        "CREATE\nLAST|P31|Q11173\n\nCREATE\nLAST|P31|Q11173"
    );
}

#[test]
fn rows_with_nothing_to_say_contribute_nothing_to_the_main_bundle() {
    let rows = vec![
        result(
            input("A", "CCO", None, None),
            CurationStatus::ExistingComplete,
            &[],
            &[],
        ),
        result(
            input("B", "CCN", None, None),
            CurationStatus::NewCompound,
            &[],
            &["B1"],
        ),
    ];
    let bundle = build_quickstatements_bundle(&rows);
    assert_eq!(bundle.main.as_ref(), "B1", "no leading blank lines");
}

#[test]
fn an_empty_result_set_produces_an_empty_bundle() {
    let bundle = build_quickstatements_bundle(&[]);
    assert_eq!(bundle, QuickStatementsBundle::default());
    assert!(bundle.dependencies.is_empty());
    assert!(bundle.main.is_empty());
}
