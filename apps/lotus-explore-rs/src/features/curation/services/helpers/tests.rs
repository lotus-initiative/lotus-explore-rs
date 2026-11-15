// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for curation `helpers`, in their own file.

// The fixtures are SPARQL prefixes and Turtle, where `#` introduces a
// comment and a literal one would be a syntax error in the thing under test.
#![allow(clippy::needless_raw_string_hashes)]

use super::*;

#[test]
fn quickstatements_statements_preserve_smiles_and_escape_delimiters() {
    // SMILES with backslashes (stereochemistry) must NOT be double-escaped
    // See https://www.wikidata.org/wiki/Q140985706
    let smiles = r#"C/C(=C\CC/C=C(\C)CC/C=C(\C)CCC(=O)O)CC/C=C(\C)CCC(=O)O"#;
    assert_eq!(
        qs_canonical_smiles_statement("LAST", smiles, false),
        format!(r#"LAST|P233|"{smiles}""#),
        "stereochemical SMILES backslashes must not be double-escaped"
    );

    // Quotes are escaped and pipe/control characters are flattened to spaces.
    assert_eq!(
        qs_isomeric_smiles_statement("LAST", "C\"C|P31\nQ1\tX"),
        r#"LAST|P2017|"C\"C P31 Q1 X""#,
        "quotes must escape and QuickStatements delimiters must flatten to spaces"
    );

    // InChI with slashes must survive untouched
    let inchi = "InChI=1S/C8H10N4O2/c1-10-4-9-6-5(10)7(13)12(3)8(14)11(6)2/h4-6H,1-3H3";
    assert_eq!(
        qs_inchi_statement("LAST", inchi),
        format!(r#"LAST|P234|"{inchi}"|S887|Q113907573"#),
        "InChI statement preserves slashes and appends S887"
    );
}

#[test]
fn qs_mass_statement_includes_smiles_reference() {
    // Mass statement should include S887 reference "inferred from SMILES" (Q113907573)
    let stmt = qs_mass_statement("LAST", 495.20268);
    assert!(stmt.contains("P2067"));
    assert!(stmt.contains("U483261"));
    assert!(stmt.contains("S887"));
    assert!(stmt.contains("Q113907573"));
}
