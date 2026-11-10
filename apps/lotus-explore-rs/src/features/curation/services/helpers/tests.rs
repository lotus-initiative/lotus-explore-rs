// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for curation `helpers`, in their own file.

// The fixtures are SPARQL prefixes and Turtle, where `#` introduces a
// comment and a literal one would be a syntax error in the thing under test.
#![allow(clippy::needless_raw_string_hashes)]

use super::*;

#[test]
fn escape_qs_string_escapes_only_quote_newline_tab() {
    // SMILES with backslashes (stereo chemistry) should NOT be double-escaped
    // See https://www.wikidata.org/wiki/Q140985706
    let smiles = r#"C/C(=C\CC/C=C(\C)CC/C=C(\C)CCC(=O)O)CC/C=C(\C)CCC(=O)O"#;
    assert_eq!(escape_qs_string(smiles), smiles);

    // Double quotes should be escaped
    assert_eq!(escape_qs_string(r#"C"C"#), r#"C\"C"#);

    // Newlines and tabs should be escaped
    assert_eq!(escape_qs_string("C\nC"), "C\\nC");
    assert_eq!(escape_qs_string("C\tC"), "C\\tC");

    // InChI with backslashes should not be double-escaped
    let inchi = "InChI=1S/C8H10N4O2/c1-10-4-9-6-5(10)7(13)12(3)8(14)11(6)2/h4-6H,1-3H3";
    assert_eq!(escape_qs_string(inchi), inchi);
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
