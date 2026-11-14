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
            isomeric_smiles: None,
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
            isomeric_smiles: None,
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

fn new_compound(name: &str, inchikey: &str, quickstatements: &[&str]) -> CurationResultRow {
    CurationResultRow {
        input: CurationInputRow {
            name: name.into(),
            smiles: "CCO".into(),
            taxon: None,
            doi: None,
        },
        canonical_smiles: Some("CCO".into()),
        isomeric_smiles: None,
        inchikey: Some(inchikey.into()),
        inchi: None,
        formula: None,
        exact_mass: None,
        mass_warning: None,
        wikidata_qid: None,
        status: CurationStatus::NewCompound,
        note: String::new(),
        dependency_blocks: vec![],
        quickstatements: quickstatements.iter().map(|s| (*s).to_string()).collect(),
    }
}

/// One new compound found in two taxa is two rows, and each row made its own
/// `CREATE`: pasted, that is two items with one `InChIKey`. The first row keeps
/// its `CREATE` and gains the second row's occurrence; the second keeps none.
#[test]
fn one_new_compound_in_two_taxa_is_created_once_in_block_form() {
    let mut rows = vec![
        new_compound(
            "Ethanol",
            "LFQSCWFLJHTTHZ-UHFFFAOYSA-N",
            &["## Ethanol\nCREATE\n  LAST|Len|\"Ethanol\"\n  LAST|P703|Q1|S248|Q9"],
        ),
        new_compound(
            "Ethanol again",
            "LFQSCWFLJHTTHZ-UHFFFAOYSA-N",
            &["## Ethanol again\nCREATE\n  LAST|Len|\"Ethanol again\"\n  LAST|P703|Q2|S248|Q9"],
        ),
    ];

    merge_new_compound_creates(&mut rows);
    let bundle = build_quickstatements_bundle(&rows);

    assert_eq!(bundle.main.matches("CREATE").count(), 1, "{}", bundle.main);
    assert!(
        bundle.main.contains("LAST|P703|Q1|S248|Q9"),
        "{}",
        bundle.main
    );
    assert!(
        bundle.main.contains("LAST|P703|Q2|S248|Q9"),
        "{}",
        bundle.main
    );
    let later = rows.get(1);
    assert_eq!(
        later.map(|r| r.quickstatements.len()),
        Some(0),
        "the second row keeps no statements"
    );
    assert!(
        later.is_some_and(|r| r.note.contains("\"Ethanol\"")),
        "the second row says where its occurrence went: {later:?}"
    );
}

/// The web app writes one line per element rather than one block, and the
/// moved occurrence has to land inside the first row's run of `LAST` lines,
/// before anything that could create another item.
#[test]
fn one_new_compound_in_two_taxa_is_created_once_in_line_form() {
    let mut rows = vec![
        new_compound(
            "A",
            "KEY",
            &[
                "CREATE",
                "LAST|Len|\"A\"",
                "LAST|P703|Q1",
                "# trailing note",
            ],
        ),
        new_compound("B", "KEY", &["CREATE", "LAST|Len|\"B\"", "LAST|P703|Q2"]),
    ];

    merge_new_compound_creates(&mut rows);

    let statements = |idx: usize| rows.get(idx).map(|r| r.quickstatements.clone());
    assert_eq!(
        statements(0),
        Some(vec![
            "CREATE".to_owned(),
            "LAST|Len|\"A\"".to_owned(),
            "LAST|P703|Q1".to_owned(),
            "LAST|P703|Q2".to_owned(),
            "# trailing note".to_owned(),
        ]),
        "the occurrence joins the run of LAST lines"
    );
    assert_eq!(statements(1), Some(vec![]), "the second row keeps none");
}

#[test]
fn different_compounds_and_existing_items_are_left_alone() {
    let mut existing = new_compound("C", "KEY1", &["Q5|P703|Q3"]);
    existing.status = CurationStatus::ExistingNeedsUpdates;
    let mut rows = vec![
        new_compound("A", "KEY1", &["CREATE", "LAST|P703|Q1"]),
        new_compound("B", "KEY2", &["CREATE", "LAST|P703|Q2"]),
        existing,
    ];
    let before = rows.clone();

    merge_new_compound_creates(&mut rows);

    assert_eq!(rows, before);
}

/// A row with no `InChIKey` has nothing to be the same compound by, so two
/// such rows are never merged, however alike their statements look.
#[test]
fn rows_without_a_key_are_never_merged() {
    let mut rows = vec![
        new_compound("A", "", &["CREATE", "LAST|P703|Q1"]),
        new_compound("B", "  ", &["CREATE", "LAST|P703|Q2"]),
    ];
    let before = rows.clone();

    merge_new_compound_creates(&mut rows);

    assert_eq!(rows, before);
}

/// The `CREATE` need not be the first element: the moved occurrence goes after
/// the `LAST` lines that follow it, not after whatever comes first.
#[test]
fn a_create_after_a_heading_still_receives_the_occurrence() {
    let mut rows = vec![
        new_compound("A", "KEY", &["## A", "CREATE", "LAST|P703|Q1"]),
        new_compound("B", "KEY", &["## B", "CREATE", "LAST|P703|Q2"]),
    ];

    merge_new_compound_creates(&mut rows);

    assert_eq!(
        rows.first().map(|r| r.quickstatements.clone()),
        Some(vec![
            "## A".to_owned(),
            "CREATE".to_owned(),
            "LAST|P703|Q1".to_owned(),
            "LAST|P703|Q2".to_owned(),
        ])
    );
}

/// The redirect is added to whatever the row already says, separated, and is
/// the whole note when there was none.
#[test]
fn the_redirect_note_joins_an_existing_note() {
    let mut with_note = new_compound("B", "KEY", &["CREATE", "LAST|P703|Q2"]);
    with_note.note = "the taxon is a genus".into();
    let mut rows = vec![
        new_compound("A", "KEY", &["CREATE", "LAST|P703|Q1"]),
        with_note,
        new_compound("C", "KEY", &["CREATE", "LAST|P703|Q3"]),
    ];

    merge_new_compound_creates(&mut rows);

    let redirect = "the same new compound as \"A\"; its occurrence is in that row's statements";
    assert_eq!(
        rows.get(1).map(|r| r.note.clone()),
        Some(format!("the taxon is a genus; {redirect}"))
    );
    assert_eq!(
        rows.get(2).map(|r| r.note.clone()),
        Some(redirect.to_owned())
    );
}
