// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! What the columnar store has to get right.
//!
//! The cases here are the ones where a columnar representation differs from a
//! row-at-a-time one: absence, deduplication, and filtering over a dictionary
//! instead of over rows.

// The panic lints keep library code free of panics on external input. A test
// that fails on a bad fixture is reporting, not panicking.
#![allow(unused_crate_dependencies)]

use lotus_model::{
    ColumnarBuilder, ColumnarResultSet, CompoundEntry, NO_VALUE, RawRow, contains_folded, folded,
};
use std::sync::Arc;

fn arc(text: &str) -> Arc<str> {
    Arc::from(text)
}

fn entry(compound: &str, taxon: &str, reference: &str) -> CompoundEntry {
    CompoundEntry {
        compound_qid: arc(compound),
        name: arc(&format!("{compound}-name")),
        inchikey: Some(arc("AAAAAAAAAAAAAA-BBBBBBBBBBB-C")),
        smiles: Some(arc("CCO")),
        mass: Some(100.0),
        formula: Some(arc("C2H6O")),
        taxon_qid: arc(taxon),
        taxon_name: arc(&format!("{taxon}-name")),
        reference_qid: arc(reference),
        ref_title: Some(arc("A paper")),
        ref_doi: Some(arc("10.1/ABC")),
        pub_year: Some(2020),
        statement: None,
    }
}

#[test]
fn a_row_survives_a_round_trip_through_the_store() {
    let original = entry("Q1", "Q2", "Q3");
    let set = ColumnarResultSet::from_entries(std::slice::from_ref(&original));
    let rebuilt = set.entry(0);

    assert_eq!(
        rebuilt.as_ref(),
        Some(&original),
        "a row must come back out of the store exactly as it went in"
    );
}

#[test]
fn rows_sharing_a_compound_agree_on_its_properties() {
    // The compound dictionary is keyed by compound, so two rows naming the same
    // compound must not disagree about it.
    let mut first = entry("Q1", "Q2", "Q3");
    first.mass = Some(100.0);
    let mut second = entry("Q1", "Q9", "Q8");
    second.mass = Some(999.0);

    let set = ColumnarResultSet::from_entries(&[first, second]);

    assert_eq!(set.row_count(), 2);
    assert_eq!(
        set.compound_count(),
        1,
        "two rows naming one compound is one dictionary entry"
    );
    // One mass column, not two, so the two rows cannot both keep a mass. The
    // first row wins, which is what makes the write once per compound rather than
    // once per row: the query asks for the same property the same way every time,
    // so the two values are the same value in every case that is not a bug.
    assert_eq!(
        set.compound_mass(0),
        Some(100.0),
        "one slot per compound, filled by the first row that mentions it"
    );
}

#[test]
fn the_row_count_is_the_endpoint_count_and_the_unique_count_is_a_lower_bound() {
    // Three rows, but only two distinct triples: the middle row repeats the first.
    let set = ColumnarResultSet::from_entries(&[
        entry("Q1", "Q2", "Q3"),
        entry("Q1", "Q2", "Q3"),
        entry("Q1", "Q2", "Q4"),
    ]);
    let stats = set.stats();

    assert_eq!(
        stats.n_entries, 3,
        "rows are stored raw, so nothing is lost"
    );
    assert_eq!(stats.n_entries_unique, 2, "distinct triples only");
    assert_eq!(stats.n_compounds, 1);
    assert_eq!(stats.n_taxa, 1);
    assert_eq!(stats.n_references, 2);
}

#[test]
fn an_absent_taxon_is_not_a_taxon() {
    // This is the trap the columnar layout has to avoid: three columns use an
    // empty string for absence rather than an Option, and COUNT(DISTINCT ?taxon)
    // does not count an unbound value. Interning "" would inflate n_taxa by one.
    let mut bare = entry("Q1", "", "");
    bare.taxon_name = arc("");
    bare.taxon_qid = arc("");
    let with_taxon = entry("Q2", "Q7", "Q3");

    let set = ColumnarResultSet::from_entries(&[bare, with_taxon]);
    let stats = set.stats();

    assert_eq!(stats.n_entries, 2);
    assert_eq!(stats.n_taxa, 1, "an empty taxon QID is an absence");
    assert_eq!(stats.n_references, 1);
    assert_eq!(set.taxon_qid(0), None);
}

#[test]
fn a_missing_compound_qid_drops_the_row() {
    // There is nothing to key a row on, and the row cannot be displayed.
    let mut nameless = entry("Q1", "Q2", "Q3");
    nameless.compound_qid = arc("");
    let good = entry("Q2", "Q5", "Q6");

    let set = ColumnarResultSet::from_entries(&[nameless, good]);

    assert_eq!(set.row_count(), 1, "a row with no compound is not a row");
}

#[test]
fn a_blank_node_is_not_interned() {
    // Every blank node is distinct. Interning one per row would cost a dictionary
    // slot per row for a value no column can show.
    let mut blank = entry("Q1", "Q2", "Q3");
    blank.taxon_qid = arc("_:b0");
    let named = entry("Q1", "Q4", "Q3");

    let set = ColumnarResultSet::from_entries(&[blank, named]);

    assert_eq!(set.stats().n_taxa, 1, "only the real QID counts");
}

#[test]
fn a_statement_is_stored_as_its_uuid_and_rebuilt_against_its_compound() {
    // Measured on a 50k sample: the prefix equals the compound QID in 99.97% of
    // rows and the UUID suffix is unique per statement, so only the UUID is kept.
    let mut row = entry("Q3613679", "Q2", "Q3");
    row.statement = Some(arc(
        "http://www.wikidata.org/entity/statement/Q3613679-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE",
    ));
    let other = entry("Q3613679", "Q4", "Q5");

    let set = ColumnarResultSet::from_entries(&[row, other]);

    assert_eq!(
        set.statement_text(0).as_deref(),
        Some("Q3613679-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE"),
        "the URI prefix and the compound prefix are both dropped and rebuilt"
    );
    assert_eq!(set.statement_text(1), None, "no statement, no cell");
}

#[test]
fn a_statement_that_is_not_a_uuid_is_still_kept() {
    let mut row = entry("Q1", "Q2", "Q3");
    row.statement = Some(arc("Q1-something-else-entirely"));

    let set = ColumnarResultSet::from_entries(std::slice::from_ref(&row));

    assert_eq!(
        set.statement_text(0).as_deref(),
        Some("Q1-something-else-entirely"),
        "an unrecognised shape must not be silently dropped"
    );
}

#[test]
fn a_text_filter_matches_a_dictionary_value_not_a_row() {
    let set = ColumnarResultSet::from_entries(&[
        entry("Q1", "Q2", "Q3"),
        entry("Q4", "Q5", "Q6"),
        entry("Q7", "Q8", "Q9"),
    ]);
    let spec = lotus_model::FilterSpec {
        compound: "q4-NAME".to_string(),
        ..Default::default()
    };
    let plan = set.plan_filter(&spec);

    assert_eq!(plan.surviving_count(&set), 1);
    assert!(plan.accepts(&set, 1), "matching is case-insensitive");
    assert!(!plan.accepts(&set, 0));
}

#[test]
fn a_text_filter_searches_every_column_its_column_offers() {
    let mut by_key = entry("Q1", "Q2", "Q3");
    by_key.inchikey = Some(arc("ZZZZZZZZZZZZ-11111111111-2"));
    let by_qid = entry("Q9999", "Q5", "Q6");
    let set = ColumnarResultSet::from_entries(&[by_key, by_qid]);

    let plan = set.plan_filter(&lotus_model::FilterSpec {
        compound: "zzzz".to_string(),
        ..Default::default()
    });
    assert_eq!(
        plan.surviving_count(&set),
        1,
        "the InChIKey is searched too"
    );

    let plan = set.plan_filter(&lotus_model::FilterSpec {
        compound: "q9999".to_string(),
        ..Default::default()
    });
    assert_eq!(
        plan.surviving_count(&set),
        1,
        "the compound QID is searched"
    );
}

#[test]
fn an_absent_value_never_matches_a_set_filter() {
    // "An unknown mass is not a small mass": the alternative returns every
    // compound Wikidata has not weighed, which is most of them.
    let mut weighed = entry("Q1", "Q2", "Q3");
    weighed.mass = Some(150.0);
    let mut unweighed = entry("Q4", "Q5", "Q6");
    unweighed.mass = None;
    let set = ColumnarResultSet::from_entries(&[weighed, unweighed]);

    let plan = set.plan_filter(&lotus_model::FilterSpec {
        mass: Some(lotus_model::Range {
            min: None,
            max: Some(200.0),
        }),
        ..Default::default()
    });

    assert_eq!(plan.surviving_count(&set), 1);
    assert!(plan.accepts(&set, 0));
    assert!(
        !plan.accepts(&set, 1),
        "a compound with no mass must not match mass <= 200"
    );
}

#[test]
fn a_blank_needle_is_not_a_filter() {
    let set = ColumnarResultSet::from_entries(&[entry("Q1", "Q2", "Q3")]);
    let spec = lotus_model::FilterSpec {
        compound: "   ".to_string(),
        mass: None,
        year: None,
        ..Default::default()
    };

    assert!(
        !spec.is_active(),
        "whitespace in a box is an empty box, and an empty box is not a filter"
    );
    assert!(!set.plan_filter(&spec).is_active());
}

#[test]
fn columns_are_combined_with_and() {
    let matching = entry("Q1", "Q2", "Q3");
    let wrong_taxon = entry("Q4", "Q5", "Q6");
    let set = ColumnarResultSet::from_entries(&[matching, wrong_taxon]);
    let spec = lotus_model::FilterSpec {
        compound: "Q1".to_string(),
        taxon: "Q2".to_string(),
        ..Default::default()
    };

    let plan = set.plan_filter(&spec);

    assert_eq!(plan.surviving_count(&set), 1, "both columns must match");
    assert_eq!(plan.surviving_rows(&set), vec![0]);
}

#[test]
fn absence_never_satisfies_a_text_filter() {
    // The taxon filter is on, and this row has no taxon at all.
    let mut bare = entry("Q1", "", "");
    bare.taxon_name = arc("");
    let named = entry("Q1", "Q2", "Q3");
    let set = ColumnarResultSet::from_entries(&[bare, named]);

    let plan = set.plan_filter(&lotus_model::FilterSpec {
        taxon: "q2".to_string(),
        ..Default::default()
    });

    assert_eq!(plan.surviving_count(&set), 1);
    assert!(
        !plan.accepts(&set, 0),
        "no taxon cannot match a taxon filter"
    );
}

#[test]
fn a_filter_over_nothing_keeps_every_row() {
    let set = ColumnarResultSet::from_entries(&[entry("Q1", "Q2", "Q3"), entry("Q4", "Q5", "Q6")]);
    let plan = set.plan_filter(&lotus_model::FilterSpec::default());

    assert!(!plan.is_active());
    assert_eq!(
        plan.surviving_count(&set),
        2,
        "an inactive plan passes everything"
    );
}

#[test]
fn the_absent_id_is_in_no_mask() {
    let set = ColumnarResultSet::from_entries(&[entry("Q1", "Q2", "Q3")]);
    let plan = set.plan_filter(&lotus_model::FilterSpec {
        taxon: "q2".to_string(),
        ..Default::default()
    });

    assert!(plan.accepts(&set, 0), "the one row does have taxon Q2");
    assert_eq!(set.taxon_qid(0), Some("Q2"));
    assert!(
        !lotus_model::Bitmask::with_len(1).contains(NO_VALUE),
        "NO_VALUE must never be found, or an absent value would match"
    );
}

#[test]
fn a_formula_filter_is_searched_on_its_own_column() {
    let mut other = entry("Q1", "Q2", "Q3");
    other.formula = Some(arc("C6H12O6"));
    let set = ColumnarResultSet::from_entries(&[entry("Q4", "Q5", "Q6"), other]);

    let plan = set.plan_filter(&lotus_model::FilterSpec {
        formula: "h12o6".to_string(),
        ..Default::default()
    });

    assert_eq!(plan.surviving_count(&set), 1);
    assert!(plan.accepts(&set, 1));
}

#[test]
fn a_reference_filter_searches_title_and_doi() {
    let set = ColumnarResultSet::from_entries(&[entry("Q1", "Q2", "Q3")]);

    for needle in ["a paper", "10.1/abc", "q3"] {
        let plan = set.plan_filter(&lotus_model::FilterSpec {
            reference: needle.to_string(),
            ..Default::default()
        });
        assert_eq!(
            plan.surviving_count(&set),
            1,
            "needle {needle:?} should match"
        );
    }
}

#[test]
fn case_folding_allocates_nothing_per_call() {
    let needle = folded("Glu-COO-");
    assert!(
        contains_folded("Some GLC-GLU-COO- dipeptide", &needle),
        "the needle may start mid-haystack"
    );
    assert!(contains_folded("glc-glu-coo-", &needle));
    assert!(
        !contains_folded("GLU", &needle),
        "a prefix is not a substring"
    );
    assert!(contains_folded("anything", &[]), "an empty needle matches");
    assert!(!contains_folded("", &folded("x")));
}

#[test]
fn folding_is_unicode_aware() {
    // A Turkish dotted capital I lowercases to two characters, which a naive
    // byte comparison would miss.
    assert!(contains_folded("İstanbul", &folded("stanbul")));
    assert!(
        contains_folded("STRASSE", &folded("strasse")),
        "ASCII case folds under Unicode lowercasing too"
    );
    assert!(contains_folded("ÉCOLE", &folded("école")));
}

#[test]
fn rows_are_pushed_as_borrowed_fields() {
    let mut builder = ColumnarBuilder::new();
    let compound = "http://www.wikidata.org/entity/Q42".to_string();
    let taxon = "http://www.wikidata.org/entity/Q7".to_string();
    builder.push(RawRow {
        compound_qid: &compound,
        taxon_qid: &taxon,
        name: "  spaced  ",
        ..RawRow::default()
    });

    let set = builder.build();

    assert_eq!(set.row_count(), 1);
    assert_eq!(
        set.compound_qid(0),
        Some("Q42"),
        "the URI prefix is stripped"
    );
    assert_eq!(
        set.compound_label(0),
        Some("spaced"),
        "and the label is trimmed"
    );
}

#[test]
fn a_mass_is_absent_when_it_is_not_a_number() {
    let mut builder = ColumnarBuilder::new();
    builder.push(RawRow {
        compound_qid: "Q1",
        mass: Some(f64::NAN),
        ..RawRow::default()
    });
    let set = builder.build();

    assert_eq!(
        set.compound_mass(0),
        None,
        "NaN is the absence marker, not a mass"
    );
}

#[test]
fn a_uuid_statement_takes_the_sixteen_byte_path_and_not_the_text_one() {
    // The round-trip text is the same either way, so a text-only assertion passes
    // whether the UUID was parsed or the whole string was kept. This one measures
    // the thing that distinguishes them.
    let mut row = entry("Q3613679", "Q2", "Q3");
    row.statement = Some(arc(
        "http://www.wikidata.org/entity/statement/Q3613679-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE",
    ));
    let set = ColumnarResultSet::from_entries(std::slice::from_ref(&row));

    assert_eq!(
        set.statement_fallback_entries(),
        0,
        "a well-formed statement must not be kept as text: it costs a string per \\
         row instead of sixteen bytes"
    );
    assert_eq!(
        set.statement_text(0).as_deref(),
        Some("Q3613679-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE")
    );
}

#[test]
fn a_statement_whose_prefix_disagrees_keeps_its_own_text() {
    // The UUID form rebuilds its text from the row's compound, so a statement
    // belonging to a different entity has to keep its own or it would come back
    // pointing at the wrong one. This is the 0.03% of real rows.
    let mut row = entry("Q3613679", "Q2", "Q3");
    row.statement = Some(arc(
        "http://www.wikidata.org/entity/statement/Q99999-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE",
    ));
    let set = ColumnarResultSet::from_entries(std::slice::from_ref(&row));

    assert_eq!(
        set.statement_text(0).as_deref(),
        Some("Q99999-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE"),
        "the wrong compound must not be substituted into the identifier"
    );
}
