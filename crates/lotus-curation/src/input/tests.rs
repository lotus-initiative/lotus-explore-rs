// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `input`, in their own file.

#![allow(clippy::expect_used, clippy::indexing_slicing)]

use super::*;

#[test]
fn parse_tsv_supports_expected_headers() {
    let tsv = "name\tsmiles\torganism\tdoi\nA\tCCO\tTaxon\thttps://doi.org/10.1/x\n";
    let rows = parse_tsv(tsv).expect("tsv parse");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "A");
    assert_eq!(rows[0].smiles, "CCO");
    assert_eq!(rows[0].taxon.as_deref(), Some("Taxon"));
    assert_eq!(rows[0].doi.as_deref(), Some("10.1/X"));
}

#[test]
fn parse_tsv_drops_rows_missing_either_required_field() {
    // Both fields are required, and they are required independently: a row
    // with a name but no structure, or a structure but no name, is as
    // unusable as a row with neither. Mutation testing caught the check
    // reading `name.is_empty() && smiles.is_empty()`, which keeps
    // half-empty rows and sends them on to be looked up.
    let tsv = "name\tsmiles\n\
               A\tCCO\n\
               \tCCC\n\
               B\t\n\
               \t\n\
               C\tCCN\n";
    let rows = parse_tsv(tsv).expect("tsv parse");
    assert_eq!(
        rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        vec!["A", "C"],
        "only rows with both a name and a structure should survive"
    );
}

#[test]
fn parse_tsv_ignores_a_header_row() {
    // The header is located by name, so a header line is just another row
    // and has to be dropped by the same required-field rule.
    let tsv = "name\tsmiles\nA\tCCO\n";
    let rows = parse_tsv(tsv).expect("tsv parse");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "A");
}

#[test]
fn row_key_normalizes_structure_taxon_and_doi() {
    // The taxon and the DOI are case-folded; the structure is only trimmed.
    // This used to assert the opposite for the DOI, pinning behaviour where
    // `10.1/A` and `10.1/a` were two different findings -- which is how a
    // re-cased duplicate got submitted twice.
    let row = CurationInputRow {
        name: "compound A".into(),
        smiles: " CCO ".into(),
        taxon: Some("  Voacanga africana ".into()),
        doi: Some("https://doi.org/10.1000/ABC".into()),
    };
    assert_eq!(
        row_uniqueness_key(&row),
        "CCO\tvoacanga africana\t10.1000/abc"
    );
}

/// In SMILES, case is chemistry: lower case marks an aromatic atom, and `Co`
/// is cobalt where `CO` is carbon then oxygen. Folding it made these pairs one
/// finding, and the second row of each was dropped as a duplicate.
#[test]
fn smiles_that_differ_only_in_case_are_different_findings() {
    for (a, b) in [("c1ccccc1", "C1CCCCC1"), ("[Co]", "[CO]"), ("Co", "CO")] {
        let row = |smiles: &str| CurationInputRow {
            name: "x".into(),
            smiles: smiles.into(),
            taxon: Some("Gentiana lutea".into()),
            doi: Some("10.1/a".into()),
        };
        assert_ne!(
            row_uniqueness_key(&row(a)),
            row_uniqueness_key(&row(b)),
            "{a} and {b} are different molecules"
        );
    }
}

#[test]
fn a_row_matches_itself_regardless_of_taxon_and_doi_case() {
    let lower = CurationInputRow {
        name: "Quercetin".into(),
        smiles: "CCO".into(),
        taxon: Some("Gentiana lutea".into()),
        doi: Some("10.1/a".into()),
    };
    let shouty = CurationInputRow {
        name: "Also called quercetin".into(),
        smiles: "CCO".into(),
        taxon: Some("GENTIANA LUTEA".into()),
        doi: Some("10.1/A".into()),
    };
    assert_eq!(
        row_uniqueness_key(&lower),
        row_uniqueness_key(&shouty),
        "the name is not part of the identity"
    );
}

#[test]
fn a_row_needs_both_a_name_and_a_smiles() {
    // The row is dropped if either half is missing. Each is dropped on its
    // own here, because the guard is an `||`: turned into `&&`, a row with
    // only one half survives and gets submitted with nothing to label it by.
    let tsv = "organism\tname\tsmiles\n\
               Taxon\tKeep\tCCO\n\
               Taxon\t\tCCC\n\
               \tNoStructure\t\n\
               Taxon\tBoth blank\t \n";
    let rows = parse_tsv(tsv).expect("tsv parse");
    let kept: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(kept, ["Keep"], "only the complete row survives: {kept:?}");
}
/// The eight-row example a chemist would actually paste, all columns filled.
///
/// Eight rows rather than one because the reader is the point: a real corpus is
/// a column of names and a column of structures, and a parser that drops or
/// shifts one row in a long column is not caught by a single-row fixture.
#[test]
fn a_pasted_multiline_corpus_keeps_every_row_and_column() {
    let tsv = "name\tsmiles\ttaxon\tdoi\n2'-deoxyguanosine\tC1[C@@H]([C@H](O[C@H]1N2C=NC3=C2N=C(NC3=O)N)CO)O\tIsaria cicadae\t10.1177/1934578X1501001233\nthymidine\tCC1=CN(C(=O)NC1=O)[C@H]2C[C@@H]([C@H](O2)CO)O\tIsaria cicadae\t10.1177/1934578X1501001233\nadenosine\tC1=NC(=C2C(=N1)N(C=N2)[C@H]3[C@@H]([C@@H]([C@H](O3)CO)O)O)N\tIsaria cicadae\t10.1177/1934578X1501001233\n2'-deoxyadenosine\tC1[C@@H]([C@H](O[C@H]1N2C=NC3=C(N=CN=C32)N)CO)O\tIsaria cicadae\t10.1177/1934578X1501001233\ntryptophan\tC1=CC=C2C(=C1)C(=CN2)C[C@@H](C(=O)O)N\tIsaria cicadae\t10.1177/1934578X1501001233\nphenylalanine\tC1=CC=C(C=C1)C[C@@H](C(=O)O)N\tIsaria cicadae\t10.1177/1934578X1501001233\ntyrosine\tC1=CC(=CC=C1C[C@@H](C(=O)O)N)O\tIsaria cicadae\t10.1177/1934578X1501001233\nN-acetylnoradrenaline\tCC(=O)NCC(C1=CC(=C(C=C1)O)O)O\tIsaria cicadae\t10.1177/1934578X1501001233";

    let rows = parse_tsv(tsv).expect("tsv parse");
    assert_eq!(rows.len(), 8, "every row in the example survives");

    for row in rows {
        assert!(!row.name.is_empty(), "a row with no name cannot be curated");
        assert!(
            !row.smiles.is_empty(),
            "a row with no structure cannot be curated"
        );
        assert_eq!(
            row.taxon.as_deref(),
            Some("Isaria cicadae"),
            "the taxon column did not shift into another field"
        );
        assert_eq!(
            row.doi.as_deref(),
            Some("10.1177/1934578X1501001233"),
            "the DOI column did not shift into another field"
        );
    }
}

#[test]
fn a_doi_url_with_nothing_after_it_is_no_doi_at_all() {
    for value in [
        "https://doi.org/",
        "doi.org/",
        "https://DOI.ORG/",
        "DOI.ORG/",
    ] {
        assert_eq!(
            normalize_doi(value),
            None,
            "{value:?} names no DOI and must not become an empty one"
        );
    }
    // Whitespace after the marker is a DOI whose canonical form is blank.
    assert_eq!(
        normalize_doi("https://doi.org/   "),
        None,
        "a marker with only whitespace behind it names no DOI"
    );

    // The marker with something behind it is a DOI, folded to canonical form and
    // upper-cased -- which is what makes `10.1/A` and `10.1/a` one finding rather
    // than two.
    assert_eq!(
        normalize_doi("https://doi.org/10.1000/ABC").as_deref(),
        Some("10.1000/ABC")
    );
    assert_eq!(normalize_doi("  10.1/x  ").as_deref(), Some("10.1/X"));
    assert_eq!(normalize_doi("   ").as_deref(), None);
}

#[test]
fn an_upper_case_doi_url_strips_the_host_and_matches_lower_case() {
    let tsv = "name\tsmiles\torganism\tdoi\nA\tCCO\tTaxon\thttps://DOI.ORG/10.1/x\n";
    let rows = parse_tsv(tsv).expect("the tsv parses");

    assert_eq!(
        rows[0].doi.as_deref(),
        Some("10.1/X"),
        "an upper-case doi.org URL has its prefix stripped and is normalised to the bare DOI"
    );

    let lower = parse_tsv("name\tsmiles\torganism\tdoi\nA\tCCO\tTaxon\thttps://doi.org/10.1/x\n")
        .expect("the tsv parses");
    assert_eq!(
        rows[0].doi, lower[0].doi,
        "host case does not split one reference into two submissions"
    );
}

/// Spreadsheet programs save "UTF-8 with BOM" by default. The mark is not part
/// of the first column's name: read as one, a leading `taxon` or `doi` column
/// was silently ignored, and a leading `name` was reported missing.
#[test]
fn a_byte_order_mark_does_not_rename_the_first_column() {
    let tsv = "\u{feff}taxon\tname\tsmiles\nGentiana lutea\tEthanol\tCCO\n";
    let rows = parse_tsv(tsv).expect("tsv parse");
    assert_eq!(
        rows.first().and_then(|row| row.taxon.as_deref()),
        Some("Gentiana lutea"),
        "{rows:?}"
    );

    let tsv = "\u{feff}name\tsmiles\nEthanol\tCCO\n";
    assert_eq!(
        parse_tsv(tsv)
            .expect("a leading name column is found")
            .len(),
        1
    );
}
