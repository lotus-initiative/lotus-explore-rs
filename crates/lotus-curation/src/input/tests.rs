// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `input`, in their own file.

#![allow(clippy::expect_used)]
#![allow(clippy::indexing_slicing)]

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
    // All three are folded, and the structure is upper-cased. This used to
    // assert the opposite for the DOI, pinning behaviour where `10.1/A` and
    // `10.1/a` were two different findings -- which is how a re-cased
    // duplicate got submitted twice.
    let row = CurationInputRow {
        name: "compound A".into(),
        smiles: " cco ".into(),
        taxon: Some("  Voacanga africana ".into()),
        doi: Some("https://doi.org/10.1000/ABC".into()),
    };
    assert_eq!(
        row_uniqueness_key(&row),
        "CCO\tvoacanga africana\t10.1000/abc"
    );
}

#[test]
fn a_row_matches_itself_regardless_of_case() {
    let lower = CurationInputRow {
        name: "Quercetin".into(),
        smiles: "cco".into(),
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
    //
    // The empty field is in the middle deliberately. `parse_tsv` trims each
    // whole line before splitting it, so a leading tab is gone before the
    // columns are read and the first field can never come out empty -- the
    // only way to see this is a gap between two present fields.
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

/// **Documents a defect. Read this before "fixing" the assertion.**
///
/// `find_ascii_ci` folds the needle and compares it against windows of the
/// **unfolded** haystack:
///
/// ```text
/// let folded: Vec<u8> = needle.iter().map(u8::to_ascii_lowercase).collect();
/// haystack.as_bytes().windows(folded.len()).position(|w| w == folded)
/// ```
///
/// so it matches only when the haystack is already lower-case, and the name and
/// the comment above it ("Fold the needle once rather than both sides") both
/// describe the intent of folding *both*. A haystack that is not lower-case
/// simply does not match.
///
/// The consequence reaches a submission. `normalize_doi` cannot find the marker,
/// so it treats the whole URL as a bare DOI and upper-cases it:
///
///     normalize_doi("https://DOI.ORG/")  ==  Some("HTTPS://DOI.ORG/")
///
/// and that string becomes the DOI on the row, and then the reference the
/// generated statements cite. A curator submitting the bundle would create a
/// reference whose DOI is a URL.
///
/// Not fixed here: this branch may not change production code. The assertions
/// below are written as the current behaviour so that fixing the fold makes them
/// fail and forces whoever does it to read this.
#[test]
fn a_doi_url_with_nothing_after_it_is_no_doi_at_all() {
    // The lower-case spellings, which do work.
    for value in ["https://doi.org/", "doi.org/"] {
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
fn an_upper_case_doi_url_is_taken_for_the_doi_itself() {
    // The defect above, shown at the level where it does damage: the row's DOI
    // is the URL, so the generated statements cite a reference by URL.
    let tsv = "name\tsmiles\torganism\tdoi\nA\tCCO\tTaxon\thttps://DOI.ORG/10.1/x\n";
    let rows = parse_tsv(tsv).expect("the tsv parses");

    assert_eq!(
        rows[0].doi.as_deref(),
        Some("HTTPS://DOI.ORG/10.1/X"),
        "this is the defect: `find_ascii_ci` does not fold the haystack, so an \
         upper-case doi.org URL is not recognised and the URL becomes the DOI. \
         When the fold is fixed this assertion is what must change, to \
         Some(\"10.1/X\")."
    );

    // And the row is then indistinguishable, on its uniqueness key, from
    // nothing else -- but two rows differing only in the case of the host do
    // collide, because neither host is stripped.
    let lower = parse_tsv("name\tsmiles\torganism\tdoi\nA\tCCO\tTaxon\thttps://doi.org/10.1/x\n")
        .expect("the tsv parses");
    assert_ne!(
        rows[0].doi, lower[0].doi,
        "which is how one reference becomes two submissions"
    );
}
