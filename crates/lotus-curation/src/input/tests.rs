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
        status: None,
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
            status: None,
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
        status: None,
    };
    let shouty = CurationInputRow {
        name: "Also called quercetin".into(),
        smiles: "CCO".into(),
        taxon: Some("GENTIANA LUTEA".into()),
        doi: Some("10.1/A".into()),
        status: None,
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

#[test]
fn parse_tabular_accepts_csv_rfc4180_quotes_and_aliases() {
    let csv = "\u{feff}\"compound_name\",\"isomeric_smiles\",\"source_organism\",\"reference_doi\"\r\n\
               \"Voatriafricanine A, \"\"alpha\"\"\",CCO,\"Voacanga africana\",https://doi.org/10.1021/acs.jnatprod.1c00812\r\n\
               \"Taxifolin\",C1=CC(=C(C=C1[C@@H]2[C@H](C(=O)C3=C(C=C(C=C3O2)O)O)O)O)O,Larix decidua,\r\n";
    let rows = crate::parse_tabular(csv).expect("csv with quotes and aliases parses");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, "Voatriafricanine A, \"alpha\"");
    assert_eq!(rows[0].smiles, "CCO");
    assert_eq!(rows[0].taxon.as_deref(), Some("Voacanga africana"));
    assert_eq!(rows[0].doi.as_deref(), Some("10.1021/ACS.JNATPROD.1C00812"));
    assert_eq!(rows[1].taxon.as_deref(), Some("Larix decidua"));
    assert_eq!(rows[1].doi, None);
}

#[test]
fn parse_tabular_keeps_tsv_with_commas_in_headers_or_cells() {
    let tsv = "name\tsmiles\tnotes, extra\tspecies\n\
               \"Alpha, beta\"\tCCO\tnote, here\tGentiana lutea\n";
    let rows = crate::parse_tabular(tsv).expect("tsv with comma in extra header parses");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Alpha, beta");
    assert_eq!(rows[0].taxon.as_deref(), Some("Gentiana lutea"));
}

#[test]
fn parse_tabular_prefers_canonical_columns_over_aliases() {
    let csv = "compound_name,name,smiles,taxon,organism\n\
               Wrong,Right,CCO,Gentiana lutea,Ignored\n";
    let rows = crate::parse_tabular(csv).expect("canonical headers take priority");
    assert_eq!(rows[0].name, "Right");
    assert_eq!(rows[0].taxon.as_deref(), Some("Gentiana lutea"));
}

#[test]
fn parse_tabular_distinguishes_quoted_delimiters_in_headers_and_handles_crlf_without_eof_newline() {
    let tsv = "\"notes, extra\"\tname\tsmiles\r\n\"note, here\"\t\"Camphor, (+)\"\tCCO";
    let tsv_rows = crate::parse_tabular(tsv).expect("quoted comma inside TSV header");
    assert_eq!(tsv_rows.len(), 1);
    assert_eq!(tsv_rows[0].name, "Camphor, (+)");

    let csv = "\"notes\textra\",name,smiles\r\n\"note\there\",\"Camphor, (+)\",CCO";
    let csv_rows = crate::parse_tabular(csv).expect("quoted tab inside CSV header");
    assert_eq!(csv_rows.len(), 1);
    assert_eq!(csv_rows[0].name, "Camphor, (+)");
}

#[test]
fn parse_tabular_prefers_tsv_when_header_widths_tie() {
    let tsv = "name\tsmiles\tnotes,a,b\nCamphor\tCCO\tnote\n";
    let rows = crate::parse_tabular(tsv).expect("tied header width keeps TSV");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Camphor");
}

#[test]
fn parse_tabular_skips_hash_comment_lines() {
    let tsv = "compound_number\tname\tsmiles\n\
               # Example: 1\tSubtortin A\tCCO\n\
                 # Indented example\tSubtortin B\tCCC\n\
               2\tSubtortin C\tCCCN\n";
    let rows = crate::parse_tabular(tsv).expect("comment lines are skipped cleanly");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Subtortin C");
    assert_eq!(rows[0].smiles, "CCCN");
}

#[test]
fn parse_tabular_reads_status_organism_name_and_bundled_example_tsv() {
    let example = include_str!("../../../../examples/curation_example.tsv");
    let rows = crate::parse_tabular(example).expect("example curation TSV parses cleanly");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].name, "carnosol");
    assert_eq!(rows[0].taxon.as_deref(), Some("Salvia rosmarinus"));
    assert_eq!(rows[0].status.as_deref(), Some("confirmed"));
    assert_eq!(rows[2].name, "aspirin");
    assert_eq!(rows[2].status.as_deref(), Some("revised"));
}

#[test]
fn parse_tabular_accepts_ten_column_jnp_template() {
    let tsv = "compound_number\tname\tsmiles\torganism\tstatus\tevidence\thost\tvoucher_or_strain\tpart\tdoi\n\
               1\tSubtortin A\tCCO\tCortinarius subtortus\tnew\tisolated\t\tGENT-2026-01\tbasidiocarp\thttps://doi.org/10.1021/acs.jnatprod.6c00850\n\
               2\tCarnosol\tCCC\tSalvia rosmarinus\tknown\tdetected\t\t\troot\t10.1271/bbb1961.47.353\n";
    let rows = crate::parse_tabular(tsv).expect("10-column JNP template TSV parses cleanly");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, "Subtortin A");
    assert_eq!(rows[0].smiles, "CCO");
    assert_eq!(rows[0].taxon.as_deref(), Some("Cortinarius subtortus"));
    assert_eq!(rows[0].status.as_deref(), Some("new"));
    assert_eq!(rows[0].doi.as_deref(), Some("10.1021/ACS.JNATPROD.6C00850"));
    assert_eq!(rows[1].name, "Carnosol");
    assert_eq!(rows[1].taxon.as_deref(), Some("Salvia rosmarinus"));
    assert_eq!(rows[1].status.as_deref(), Some("known"));
    assert_eq!(rows[1].doi.as_deref(), Some("10.1271/BBB1961.47.353"));
}

#[test]
fn parse_tabular_handles_deterministic_quoted_csv_and_tsv_rows() {
    let taxa = ["Salvia rosmarinus", "Gentiana lutea", "Taxus baccata"];
    for i in 0..128_u32 {
        let h = i.wrapping_mul(2_654_435_761) ^ (i >> 3);
        let suffix = h % 10_000;
        let name = format!("Compound-{suffix}, alpha\"beta");
        let smiles = format!(
            "C{}O",
            "C".repeat(usize::try_from((h >> 8) % 5 + 1).unwrap_or(1))
        );
        let taxon = taxa[usize::try_from(h).unwrap_or(0) % taxa.len()];
        let doi_raw = format!("https://doi.org/10.{}/lotus.{suffix:x}", 1000 + (h % 8000));
        let expected_doi = format!("10.{}/LOTUS.{suffix:X}", 1000 + (h % 8000));
        let newline = if i % 2 == 0 { "\n" } else { "\r\n" };
        let quoted_name = format!("\"{}\"", name.replace('"', "\"\""));

        let csv = format!(
            "name,smiles,organism,doi{newline}{quoted_name},{smiles},{taxon},{doi_raw}{newline}"
        );
        let csv_rows = parse_tabular(&csv).expect("deterministic CSV parses");
        assert_eq!(csv_rows.len(), 1);
        assert_eq!(csv_rows[0].name, name);
        assert_eq!(csv_rows[0].smiles, smiles);
        assert_eq!(csv_rows[0].taxon.as_deref(), Some(taxon));
        assert_eq!(csv_rows[0].doi.as_deref(), Some(expected_doi.as_str()));

        let tsv = format!(
            "name\tsmiles\torganism\tdoi{newline}{quoted_name}\t{smiles}\t{taxon}\t{doi_raw}{newline}"
        );
        let tsv_rows = parse_tabular(&tsv).expect("deterministic TSV parses");
        assert_eq!(
            tsv_rows, csv_rows,
            "CSV and TSV parse identically for case {i}"
        );
    }
}
