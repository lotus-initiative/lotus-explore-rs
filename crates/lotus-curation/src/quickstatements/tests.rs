// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `quickstatements`, in their own file.

use super::*;
use crate::{CurationInputRow, CurationStatus};

#[test]
fn escape_quotes_so_a_value_cannot_end_its_own_scalar() {
    // A `Say "hi"` compound would otherwise write a property named `hi`,
    // because the format is pipe-separated with `"`-quoted scalars.
    assert_eq!(escape_quickstatements(r#"Say "hi""#), r#"Say \"hi\""#);
    assert_eq!(
        escape_quickstatements(r#""""#),
        r#"\"\""#,
        "two quotes become two escaped quotes"
    );
    assert_eq!(
        escape_quickstatements(r#"a"b"c"#),
        r#"a\"b\"c"#,
        "every quote is escaped, not just the first"
    );
}

#[test]
fn escape_flattens_whitespace_that_would_break_the_line() {
    // A newline inside a scalar ends the statement, so it becomes a space.
    assert_eq!(escape_quickstatements("a\nb"), "a b");
    assert_eq!(escape_quickstatements("a\rb"), "a b");
    assert_eq!(escape_quickstatements("a\tb"), "a b");
    assert_eq!(
        escape_quickstatements("line one\nline two\n"),
        "line one line two ",
        "each control character is replaced, including trailing ones"
    );
}

#[test]
fn escape_leaves_ordinary_values_untouched() {
    assert_eq!(escape_quickstatements("Quercetin"), "Quercetin");
    assert_eq!(escape_quickstatements("CCO"), "CCO");
    assert_eq!(escape_quickstatements(""), "");
    assert_eq!(
        escape_quickstatements("Voacanga africana"),
        "Voacanga africana",
        "spaces are not control characters and must survive"
    );
}

#[test]
fn deduplicates_dependencies_and_joins_sections() {
    let rows = vec![
        CurationResultRow {
            input: CurationInputRow {
                name: "A".into(),
                smiles: "C".into(),
                taxon: None,
                doi: None,
            },
            canonical_smiles: None,
            inchikey: None,
            inchi: None,
            formula: None,
            exact_mass: None,
            mass_warning: None,
            wikidata_qid: None,
            status: CurationStatus::NewCompound,
            note: String::new(),
            dependency_blocks: vec!["DEP-1".into(), "DEP-1".into()],
            quickstatements: vec!["MAIN-1A".into(), "MAIN-1B".into()],
        },
        CurationResultRow {
            input: CurationInputRow {
                name: "B".into(),
                smiles: "N".into(),
                taxon: None,
                doi: None,
            },
            canonical_smiles: None,
            inchikey: None,
            inchi: None,
            formula: None,
            exact_mass: None,
            mass_warning: None,
            wikidata_qid: None,
            status: CurationStatus::NewCompound,
            note: String::new(),
            dependency_blocks: vec!["DEP-1".into(), "DEP-2".into()],
            quickstatements: vec!["MAIN-2".into()],
        },
    ];

    let bundle = build_quickstatements_bundle(&rows);
    assert_eq!(bundle.dependencies.as_ref(), "DEP-1\n\nDEP-2");
    assert_eq!(bundle.main.as_ref(), "MAIN-1A\nMAIN-1B\n\nMAIN-2");
}
