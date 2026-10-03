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
        set.stats().n_compounds,
        1,
        "two rows naming one compound is one compound"
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
    assert_eq!(set.taxon_qid(0), Some(2), "a numeric QID is what is stored");
    assert_eq!(
        set.taxon_qid_text(0).as_deref(),
        Some("Q2"),
        "and the text is rendered back to what the cell held"
    );
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
        set.compound_qid_text(0).as_deref(),
        Some("Q42"),
        "the URI prefix is stripped and the QID renders back identically"
    );
    assert_eq!(
        set.compound_qid(0),
        Some(42),
        "and it is stored as the number"
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

#[test]
fn a_stored_qid_renders_back_to_exactly_what_the_cell_held() {
    // The reason the numeric store is safe: a QID is a number with a `Q` on the
    // front, and the round trip has to be exact or every Wikidata link is a 404.
    // Covers a bare id, a full URI, a bare-integer projection, and the range of
    // magnitudes Wikidata actually uses.
    for cell in [
        "Q1",
        "Q42",
        "Q3613679",
        "http://www.wikidata.org/entity/Q3613679",
        "https://www.wikidata.org/entity/Q3613679",
        "3613679",
    ] {
        let row = CompoundEntry {
            compound_qid: arc(cell),
            ..entry("unused", "Q2", "Q3")
        };
        let set = ColumnarResultSet::from_entries(std::slice::from_ref(&row));
        let expected = cell.rsplit('/').next().unwrap_or(cell);
        let expected = expected.strip_prefix('Q').unwrap_or(expected);
        let expected = format!("Q{expected}");

        assert_eq!(
            set.compound_qid_text(0).as_deref(),
            Some(expected.as_str()),
            "cell {cell:?} must render back as {expected:?}"
        );
    }
}

#[test]
fn a_qid_filter_still_matches_the_text_a_reader_types() {
    // The compound filter's QID arm renders through a stack buffer rather than
    // holding the text, so this is the test that the buffer is right.
    let set = ColumnarResultSet::from_entries(&[
        entry("Q3613679", "Q2", "Q3"),
        entry("Q42", "Q2", "Q3"),
        entry("Q999", "Q2", "Q3"),
    ]);

    for (needle, expected) in [("q3613679", 1), ("Q42", 1), ("3613", 1), ("q9", 1)] {
        let plan = set.plan_filter(&lotus_model::FilterSpec {
            compound: needle.to_string(),
            ..Default::default()
        });
        assert_eq!(
            plan.surviving_count(&set),
            expected,
            "needle {needle:?} should match {expected} row"
        );
    }
}

#[test]
fn a_row_costs_the_documented_number_of_bytes() {
    // The module docs quote a per-row figure, and a figure nobody measures rots.
    // This is the measurement, and the three per-row columns are the whole of it:
    // three dictionary ids and a statement.
    let set = ColumnarResultSet::from_entries(&[entry("Q1", "Q2", "Q3")]);
    let per_row = 3 * std::mem::size_of::<u32>() + std::mem::size_of::<lotus_model::StatementId>();

    assert_eq!(
        per_row, 32,
        "the module docs quote 32 bytes of id columns a row"
    );
    assert_eq!(
        set.row_count(),
        1,
        "one row, so the set's own cost is the dictionaries and nothing else"
    );
}

#[test]
fn a_repeated_statement_does_not_grow_the_fallback_column() {
    // Regression, and the shape of bug that only shows up as unexplained growth.
    //
    // The fallback column used to be given the slot index it *predicted* the value
    // would land at -- `values.len()` before the write -- and that prediction is
    // wrong for a value already interned: `set` then appended a slot pointing at
    // the existing string. One slot per *call* instead of per distinct value, so a
    // result whose statements are all the same grew by four bytes a row while its
    // strings stayed at one.
    //
    // Three million of those is twelve megabytes describing a single value, and it
    // is why the benchmark once reported 63 MB of "fallbacks" for a column that
    // held a million distinct statements.
    let mut builder = ColumnarBuilder::new();
    for _ in 0..1_000 {
        builder.push(RawRow {
            compound_qid: "Q1",
            statement: Some("http://www.wikidata.org/entity/statement/Q1-not-a-uuid"),
            ..RawRow::default()
        });
    }
    let set = builder.build();

    assert_eq!(
        set.statement_fallback_entries(),
        1,
        "one thousand identical statements, one stored string"
    );
    assert_eq!(set.row_count(), 1_000, "and every row is still there");
}

#[test]
fn two_equality_semantics_the_table_depends_on() {
    // `ResultDataState` derives `PartialEq`, so a new result set has to compare
    // unequal to the one it replaced or the table does not re-render. Comparing the
    // contents instead would mean walking millions of rows on every comparison,
    // which is why the identity is compared -- and that is only safe if a set is
    // equal to itself and nothing else.
    let first = ColumnarResultSet::from_entries(&[entry("Q1", "Q2", "Q3")]);
    let same_rows_built_again = ColumnarResultSet::from_entries(&[entry("Q1", "Q2", "Q3")]);
    let different = ColumnarResultSet::from_entries(&[entry("Q9", "Q2", "Q3")]);
    let empty = ColumnarResultSet::default();

    assert_eq!(first, first, "a set equals itself");
    assert_ne!(
        first, same_rows_built_again,
        "two sets holding the same rows are not the same set: \
         otherwise a re-fetch of an identical result would not re-render"
    );
    assert_ne!(first, different);
    assert_ne!(first, empty);
    assert_eq!(
        empty, empty,
        "and two 'no search has run' states are equal, or every reset looks \
         like a change"
    );
    assert_ne!(empty, first, "an empty state is never a result");
}

#[test]
fn every_row_reader_reads_the_field_it_names() {
    // The row accessors are what the table's sort reads, and they are read only
    // from the app -- which mutation testing does not run, because it is ~2,700
    // mutants of Dioxus rendering for a few hundred of everything else. So a
    // reader that quietly returned the wrong column would be invisible to the
    // mutant run *and* to the app's own tests. One row with every field filled and
    // one with none is what closes that.
    let full = CompoundEntry {
        compound_qid: arc("Q3613679"),
        name: arc("Quercetin"),
        inchikey: Some(arc("ABCDEF-GHIJKL-M")),
        smiles: Some(arc("O=c1cc(-c2ccccc2)oc2cc(O)cc(O)c12")),
        mass: Some(302.24),
        formula: Some(arc("C15H10O7")),
        taxon_qid: arc("Q128267"),
        taxon_name: arc("Quercus robur"),
        reference_qid: arc("Q100000001"),
        ref_title: Some(arc("Flavonoid isolation")),
        ref_doi: Some(arc("10.1000/ABC")),
        pub_year: Some(2019),
        statement: Some(arc("Q3613679-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EE")),
    };
    let blank = CompoundEntry::default();

    let set = ColumnarResultSet::from_entries(&[full, blank]);

    assert_eq!(set.inchikey(0), Some("ABCDEF-GHIJKL-M"));
    assert_eq!(set.smiles(0), Some("O=c1cc(-c2ccccc2)oc2cc(O)cc(O)c12"));
    assert_eq!(set.formula(0), Some("C15H10O7"));
    assert_eq!(set.mass(0), Some(302.24));
    assert_eq!(set.compound_label(0), Some("Quercetin"));
    assert_eq!(set.taxon_label(0), Some("Quercus robur"));
    assert_eq!(set.reference_title(0), Some("Flavonoid isolation"));
    assert_eq!(set.pub_year(0), Some(2019));
    assert_eq!(
        set.compound_qid_text(0).as_deref(),
        Some("Q3613679"),
        "and the QID readers render back what the cell held"
    );
    assert_eq!(set.taxon_qid_text(0).as_deref(), Some("Q128267"));
    assert_eq!(set.reference_qid_text(0).as_deref(), Some("Q100000001"));

    // And the empty row reads as empty for all of them, rather than as the first
    // row's values: an accessor that forgot its own column would pass the checks
    // above and fail these.
    assert_eq!(set.inchikey(1), None);
    assert_eq!(set.smiles(1), None);
    assert_eq!(set.formula(1), None);
    assert_eq!(set.mass(1), None);
    assert_eq!(set.compound_label(1), None);
    assert_eq!(set.taxon_label(1), None);
    assert_eq!(set.reference_title(1), None);
    assert_eq!(set.pub_year(1), None);

    // A row index past the end reads as absent rather than panicking, which is
    // what lets the virtualiser ask for a window it has not scrolled to yet.
    assert_eq!(set.inchikey(99), None);
    assert_eq!(set.mass(99), None);
    assert_eq!(set.pub_year(99), None);
}

#[test]
fn a_uuid_of_the_wrong_shape_is_kept_as_text_rather_than_half_read() {
    // `parse_uuid` accepts a string only when every group is the right width and
    // the hex characters pair up into whole bytes. A near-miss has to fall through
    // to the text path, because half a UUID is a different identifier and the
    // column is a link.
    for malformed in [
        // An odd number of hex characters: a half-filled final byte.
        "Q1-0D8245CF-C1C0-45AA-8994-6BEBFF6B15E", // 11 in the last group
        "Q1-0D8245CF-C1C0-45AA-8994-6BEBFF6B15EEE", // 13
        // A group of the wrong width.
        "Q1-0D8245C-C1C0-45AA-8994-6BEBFF6B15EE", // 7 in the first group
        // Too few groups: a bare identifier with no UUID at all.
        "Q1-0D8245CF",
        // Not hex.
        "Q1-0D8245CF-C1C0-45AA-8994-6BEBFF6B15ZZ",
    ] {
        let row = entry("Q1", "Q2", "Q3");
        let mut row = row;
        row.statement = Some(arc(&format!(
            "http://www.wikidata.org/entity/statement/{malformed}"
        )));
        let set = ColumnarResultSet::from_entries(std::slice::from_ref(&row));

        assert_eq!(
            set.statement_text(0).as_deref(),
            Some(malformed),
            "{malformed:?} must be kept exactly as it arrived"
        );
    }
}

#[test]
fn the_collections_report_emptiness_before_and_after_a_row() {
    // `len` and `is_empty` come in pairs because the lint profile requires it, and
    // an accessor nobody calls is an accessor nobody checks. These are the states
    // the empty case reaches.
    let empty = ColumnarResultSet::default();
    assert!(empty.is_empty());
    assert_eq!(empty.row_count(), 0);

    let one = ColumnarResultSet::from_entries(&[entry("Q1", "Q2", "Q3")]);
    assert!(!one.is_empty());
    assert_eq!(one.row_count(), 1);

    let bitmask = lotus_model::Bitmask::with_len(0);
    assert!(bitmask.is_empty());
    let mut filled = lotus_model::Bitmask::with_len(64);
    filled.insert(3);
    assert!(!filled.is_empty());
    assert_eq!(filled.len(), 1);

    let mut builder = ColumnarBuilder::new();
    assert_eq!(builder.row_count(), 0);
    builder.push(RawRow {
        compound_qid: "Q1",
        ..RawRow::default()
    });
    assert_eq!(builder.row_count(), 1);
}

#[test]
fn the_compound_dictionary_is_walkable_for_a_hash_that_must_not_change() {
    // `compound_qids` feeds the result hash, so it has to yield every distinct
    // compound once, and in a stable order: a set built twice has to hash to the
    // same digest or every shared link stops resolving.
    let rows: Vec<CompoundEntry> = ["Q7", "Q3", "Q5"]
        .iter()
        .map(|q| entry(q, "Q2", "Q3"))
        .collect();
    let set = ColumnarResultSet::from_entries(&rows);

    let first: Vec<u32> = set.compound_qids().collect();
    let rebuilt = ColumnarResultSet::from_entries(&rows);
    let second: Vec<u32> = rebuilt.compound_qids().collect();

    assert_eq!(first.len(), 3, "three compounds, counted once each");
    assert_eq!(first, second, "first-seen order is stable across builds");
    assert_eq!(
        first,
        vec![7, 3, 5],
        "and it is the order the rows arrived in, not a sort"
    );

    assert!(
        ColumnarResultSet::default()
            .compound_qids()
            .next()
            .is_none(),
        "an empty set yields nothing to hash"
    );
}

/// One criterion is enough to make a filter active.
///
/// `FilterSpec::is_active` and `FilterPlan::is_active` are both a chain of `||`,
/// and both decide something the reader sees: an inactive filter is skipped
/// entirely, so a criterion that fails to switch one on is a filter that silently
/// does nothing while the table still looks filtered somewhere else.
///
/// The suite set criteria in pairs or in isolation on the compound column, so
/// every operand to the right of the first was dead as far as any test could tell.
/// Mutation testing found all thirteen: `||` weakened to `&&` at each position,
/// plus `is_active -> false`.
#[test]
fn any_single_criterion_makes_a_filter_active() {
    use lotus_model::{FilterSpec, Range};

    let set = ColumnarResultSet::from_entries(&[entry("Q1", "Q2", "Q3")]);
    let range = Range {
        min: Some(0.0),
        max: Some(1000.0),
    };
    // One criterion at a time, each named so a failure says which one was dropped.
    let each: [(&str, FilterSpec); 6] = [
        (
            "compound",
            FilterSpec {
                compound: "Q1".to_string(),
                ..Default::default()
            },
        ),
        (
            "formula",
            FilterSpec {
                formula: "C2H6O".to_string(),
                ..Default::default()
            },
        ),
        (
            "taxon",
            FilterSpec {
                taxon: "Q2".to_string(),
                ..Default::default()
            },
        ),
        (
            "reference",
            FilterSpec {
                reference: "Q3".to_string(),
                ..Default::default()
            },
        ),
        (
            "mass",
            FilterSpec {
                mass: Some(range),
                ..Default::default()
            },
        ),
        (
            "year",
            FilterSpec {
                year: Some(range),
                ..Default::default()
            },
        ),
    ];

    for (name, spec) in each {
        assert!(
            spec.is_active(),
            "a spec with only `{name}` set constrains that column"
        );
        let plan = set.plan_filter(&spec);
        assert!(
            plan.is_active(),
            "a plan compiled from a spec with only `{name}` set must be active"
        );
    }

    // And the other half: nothing set means nothing applied. Without this, a
    // mutant that made `is_active` always true would survive.
    let empty = FilterSpec::default();
    assert!(!empty.is_active(), "an empty spec constrains nothing");
    assert!(
        !set.plan_filter(&empty).is_active(),
        "a plan over an empty spec must not be active"
    );
}

/// A nameless compound is shown as its identifier, not as nothing.
///
/// `compound_label_or_qid` has three outcomes and the suite only ever saw one of
/// them: a compound whose name is present. A mutant that returned `None`, or an
/// empty string, or any constant, passed every existing test -- which means the
/// table could show a nameless compound as a blank cell and nothing would notice.
#[test]
fn a_nameless_compound_falls_back_to_its_identifier() {
    let mut nameless = entry("Q16521", "Q2", "Q3");
    nameless.name = arc("");
    let set = ColumnarResultSet::from_entries(&[nameless, entry("Q42", "Q2", "Q3")]);

    assert_eq!(
        set.compound_label_or_qid(1),
        Some(std::borrow::Cow::Borrowed("Q42-name")),
        "a compound with a name is shown by its name"
    );
    assert_eq!(
        set.compound_label_or_qid(0),
        Some(std::borrow::Cow::Owned("Q16521".to_string())),
        "a compound with no name is shown as its QID, never as nothing"
    );
}

/// A reference DOI is returned when the reference has one, and nothing when it
/// does not.
///
/// The accessor is one `Option` lookup, and a mutant replacing it with a constant
/// survived because every fixture's reference carried a DOI. Both directions are
/// pinned here.
#[test]
fn a_reference_doi_is_read_when_present_and_absent_otherwise() {
    let mut without = entry("Q1", "Q2", "Q5");
    without.ref_doi = None;
    let set = ColumnarResultSet::from_entries(&[entry("Q1", "Q2", "Q3"), without]);

    assert_eq!(
        set.reference_doi(0),
        Some("10.1/ABC"),
        "the reference's own DOI is returned"
    );
    assert_eq!(
        set.reference_doi(1),
        None,
        "a reference with no DOI has none to return"
    );
}

/// A range includes its own bounds and excludes what is outside them.
///
/// Both edges are checked against the operator rather than against a value in the
/// middle, because `value >= min` and `value >= max` differ only at the boundary.
/// Mutation testing turned `>=` into `<` and `&&` into `||` here and both
/// survived, which says the suite never sat on an edge.
#[test]
fn a_range_includes_its_bounds_and_refuses_what_is_outside() {
    use lotus_model::Range;

    let bounded = Range {
        min: Some(10.0),
        max: Some(20.0),
    };
    assert!(bounded.accepts(Some(10.0)), "the lower bound is inside");
    assert!(bounded.accepts(Some(20.0)), "the upper bound is inside");
    assert!(bounded.accepts(Some(15.0)), "the middle is inside");
    assert!(
        !bounded.accepts(Some(9.999)),
        "below the lower bound is outside"
    );
    assert!(
        !bounded.accepts(Some(20.001)),
        "above the upper bound is outside"
    );

    // An absent value never matches: "an unknown mass is not a small mass".
    assert!(
        !bounded.accepts(None),
        "a missing value is not inside a range, whatever the range"
    );

    // An unbounded side is no constraint at all.
    let open_below = Range {
        min: None,
        max: Some(20.0),
    };
    assert!(
        open_below.accepts(Some(-1e9)),
        "with no lower bound, a very small value is inside"
    );
    assert!(
        !open_below.accepts(Some(20.001)),
        "the upper bound still applies"
    );

    let open_above = Range {
        min: Some(10.0),
        max: None,
    };
    assert!(
        open_above.accepts(Some(1e9)),
        "with no upper bound, a very large value is inside"
    );
    assert!(
        !open_above.accepts(Some(9.999)),
        "the lower bound still applies"
    );

    assert!(
        Range {
            min: None,
            max: None
        }
        .accepts(Some(15.0)),
        "a range with neither bound accepts whatever it is given"
    );
}

/// A taxon needle matches a taxon's *name* as well as its QID.
///
/// The taxon branch of `plan_filter` accepts a row when either the interned QID
/// matches or the taxon name contains the needle. The suite only ever searched by
/// QID, so the name arm was untested and mutation testing turned its `||` into
/// `&&` without anything failing: a search for a genus name would have silently
/// matched nothing at all.
#[test]
fn a_taxon_needle_matches_the_taxon_name_as_well_as_its_qid() {
    let mut named = entry("Q1", "Q9999", "Q3");
    named.taxon_name = arc("Gentiana lutea");
    let set = ColumnarResultSet::from_entries(&[named]);

    let by_name = set.plan_filter(&lotus_model::FilterSpec {
        taxon: "gentiana".to_string(),
        ..Default::default()
    });
    assert_eq!(
        by_name.surviving_count(&set),
        1,
        "a taxon name must match a needle that is not the QID"
    );

    // Case folding: the needle is folded once, and the row is matched folded.
    let mixed_case = set.plan_filter(&lotus_model::FilterSpec {
        taxon: "LUTEA".to_string(),
        ..Default::default()
    });
    assert_eq!(
        mixed_case.surviving_count(&set),
        1,
        "a taxon name must match regardless of the needle's case"
    );

    // And the QID arm still works, or the two cannot be told apart.
    let by_qid = set.plan_filter(&lotus_model::FilterSpec {
        taxon: "Q9999".to_string(),
        ..Default::default()
    });
    assert_eq!(by_qid.surviving_count(&set), 1, "the QID arm still matches");

    // A needle matching neither arm matches nothing.
    let neither = set.plan_filter(&lotus_model::FilterSpec {
        taxon: "Rosmarinus".to_string(),
        ..Default::default()
    });
    assert_eq!(neither.surviving_count(&set), 0, "no arm, no row");
}

/// `surviving_rows` returns every surviving row, not the first one.
///
/// The suite asserted `surviving_rows == vec![0]` in the one case where row 0 was
/// the only survivor, so a mutant returning a one-element vector survived. This
/// is the accessor a selection or an export walks, so a truncated list is a
/// silently missing row rather than a visible error.
#[test]
fn surviving_rows_lists_every_surviving_row() {
    let set = ColumnarResultSet::from_entries(&[
        entry("Q1", "Q2", "Q3"),
        entry("Q4", "Q2", "Q5"),
        entry("Q6", "Q7", "Q8"),
    ]);

    let plan = set.plan_filter(&lotus_model::FilterSpec {
        taxon: "Q2".to_string(),
        ..Default::default()
    });

    assert_eq!(
        plan.surviving_rows(&set),
        vec![0, 1],
        "both rows in Q2 survive, and the list is not truncated to one"
    );
}
