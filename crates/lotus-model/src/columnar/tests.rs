// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `columnar` that need its private fields and functions.

#![allow(clippy::expect_used)]

use super::*;

/// A slot-keyed column holding `cells` in slots `0..cells.len()`.
fn column(cells: &[Option<&str>]) -> SparseStrings {
    let mut strings = SparseStrings::new();
    for (slot, cell) in cells.iter().enumerate() {
        strings.set(slot, *cell);
    }
    strings
}

/// A QID dictionary holding `count` distinct QIDs.
#[cfg(feature = "diagnostics")]
fn qids(count: u32) -> QidDictionary {
    let mut dictionary = QidDictionary::new();
    for numeric in 1..=count {
        dictionary.intern(numeric);
    }
    dictionary
}

#[test]
fn a_qid_dictionary_is_empty_until_something_is_interned() {
    let mut dictionary = QidDictionary::new();
    assert!(dictionary.is_empty());
    dictionary.intern(7);
    assert!(!dictionary.is_empty());
}

#[test]
fn a_sparse_column_counts_its_slots_and_not_its_values() {
    let empty = SparseStrings::new();
    assert_eq!(empty.len(), 0);
    assert!(empty.is_empty());

    // Three slots, two values: `len` is the slot count, which is neither 0, 1 nor
    // the distinct count.
    let filled = column(&[Some("a"), None, Some("a")]);
    assert_eq!(filled.len(), 3);
    assert_eq!(filled.distinct_len(), 1);
    assert!(!filled.is_empty());
}

#[test]
fn a_qid_number_is_only_ascii_digits() {
    // `str::parse::<u32>` accepts a leading `+`, so the digit scan is what refuses
    // it, and it must refuse it whether or not the run is empty.
    assert_eq!(parse_numeric_qid("+12"), None);
    assert_eq!(parse_numeric_qid("1a"), None);
    assert_eq!(parse_numeric_qid(""), None);
    assert_eq!(parse_numeric_qid("12"), Some(12));
}

/// A set whose every dictionary has a different, hand-countable shape.
///
/// Built from fields rather than rows because the builder cannot reach every
/// shape: it never fills `statement_fallbacks`' slots, so that column's slot term
/// would always be zero, and multiplying or dividing a zero changes nothing a
/// test can see. Each count is distinct and above two so that `a * b`, `a + b`
/// and `a / b` all disagree.
#[cfg(feature = "diagnostics")]
fn shaped_set() -> ColumnarResultSet {
    ColumnarResultSet {
        compounds: qids(3),
        // slots, values, string bytes: 4, 2, 5
        compound_names: column(&[Some("ab"), Some("cde"), None, Some("ab")]),
        // 1, 1, 1
        compound_inchikeys: column(&[Some("x")]),
        // 3, 2, 7
        compound_smiles: column(&[Some("CCO"), Some("CCCO"), None]),
        // 6, 3, 10
        compound_formulas: column(&[
            None,
            Some("H2O"),
            Some("H2O"),
            Some("CO2"),
            Some("NaCl"),
            None,
        ]),
        compound_masses: vec![1.0; 4],
        taxa: qids(2),
        // 2, 1, 4
        taxon_names: column(&[Some("Rosa"), Some("Rosa")]),
        references: qids(5),
        // 5, 4, 10
        reference_titles: column(&[Some("A"), Some("BB"), Some("CCC"), Some("DDDD"), None]),
        // 2, 1, 6
        reference_dois: column(&[Some("10.1/x"), None]),
        reference_years: vec![None; 6],
        // 3, 2, 8
        statement_fallbacks: column(&[Some("abc"), Some("defgh"), None]),
        // 2, 1, 6
        reference_node_hashes: column(&[Some("a1b2c3"), None]),
        ..ColumnarResultSet::default()
    }
}

#[cfg(feature = "diagnostics")]
#[test]
fn every_dictionary_cost_is_the_documented_arithmetic() {
    // A QID entry is 16 bytes: two u32 and 8 of map slack. A string column is
    // 16 per value (an `Arc<str>`), its string bytes, and 4 per slot.
    assert_eq!(
        shaped_set().dictionary_costs(),
        vec![
            ("compound qids", 3, 48),
            ("compound names", 4, 2 * 16 + 5 + 4 * 4),
            ("compound inchikeys", 1, 16 + 1 + 4),
            ("compound smiles", 3, 2 * 16 + 7 + 3 * 4),
            ("compound formulas", 6, 3 * 16 + 10 + 6 * 4),
            ("compound masses", 4, 32),
            ("taxon qids", 2, 32),
            ("taxon names", 2, 16 + 4 + 2 * 4),
            ("reference qids", 5, 80),
            ("reference titles", 5, 4 * 16 + 10 + 5 * 4),
            ("reference dois", 2, 16 + 6 + 2 * 4),
            ("reference years", 6, 24),
            ("statement fallbacks", 2, 2 * 16 + 8 + 3 * 4),
            ("reference nodes", 2, 16 + 6 + 2 * 4),
        ]
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn the_total_is_the_sum_of_the_itemised_costs() {
    let itemised = 48 + 53 + 21 + 51 + 82 + 32 + 32 + 28 + 80 + 94 + 30 + 24 + 52 + 30;
    assert_eq!(itemised, 657);
    assert_eq!(shaped_set().total_dictionary_bytes(), 657);
}

#[test]
fn the_empty_fixture_row_is_a_compound_and_nothing_else() {
    // `lotus-query` builds its empty-cell tests on this row; it has to name a
    // compound, or those tests would exercise a row the builder never sees.
    let set = ColumnarResultSet::from_entries(&[crate::testing::empty_row()]);
    assert_eq!(set.compound_qid_text(0).as_deref(), Some("Q42"));
    assert_eq!(set.taxon_qid(0), None);
    assert_eq!(set.reference_qid(0), None);
    assert_eq!(set.inchikey(0), None);
}

#[test]
fn each_compound_takes_exactly_one_slot_in_every_compound_column() {
    let set = ColumnarResultSet::from_entries(&[
        crate::testing::full_row(),
        crate::testing::empty_row(),
        crate::testing::full_row(),
    ]);
    assert_eq!(
        set.compound_masses.len(),
        2,
        "two distinct compounds, not one per row"
    );
    assert_eq!(set.compound_names.len(), 2);
}

#[test]
fn materialising_an_entry_shares_interned_strings_by_arc() {
    let set =
        ColumnarResultSet::from_entries(&[crate::testing::full_row(), crate::testing::full_row()]);
    let first = set.entry(0).expect("row 0");
    let second = set.entry(1).expect("row 1");
    assert!(
        Arc::ptr_eq(&first.name, &second.name),
        "compound name should be Arc-cloned from SparseStrings without reallocating"
    );
    assert!(
        Arc::ptr_eq(&first.taxon_name, &second.taxon_name),
        "taxon name should be Arc-cloned from SparseStrings without reallocating"
    );
    assert!(
        Arc::ptr_eq(
            first.inchikey.as_ref().expect("inchikey"),
            second.inchikey.as_ref().expect("inchikey")
        ),
        "inchikey should be Arc-cloned from SparseStrings without reallocating"
    );
}

#[test]
fn pushing_a_row_normalizes_subscript_digits_in_the_formula() {
    let mut builder = ColumnarBuilder::new();
    builder.push(RawRow {
        compound_qid: "Q1",
        name: "Quercetin",
        inchikey: None,
        smiles: None,
        mass: None,
        formula: Some("C₁₅H₁₀O₇"),
        taxon_qid: "Q10",
        taxon_name: "",
        reference_qid: "",
        reference_node: "",
        ref_title: None,
        ref_doi: None,
        pub_year: None,
        statement: None,
    });
    builder.push(RawRow {
        compound_qid: "Q2",
        name: "Kaempferol",
        inchikey: None,
        smiles: None,
        mass: None,
        formula: Some("C15H10O6"),
        taxon_qid: "Q20",
        taxon_name: "Quercus robur",
        reference_qid: "",
        reference_node: "",
        ref_title: None,
        ref_doi: None,
        pub_year: None,
        statement: None,
    });
    let set = builder.build();
    assert_eq!(set.formula(0), Some("C15H10O7"));
    assert_eq!(set.formula(1), Some("C15H10O6"));
    assert_eq!(set.taxon_label(0), None);
    assert_eq!(
        set.taxon_label(1),
        Some("Quercus robur"),
        "an earlier taxon without a scientific name must not drop later taxon names"
    );
}

#[test]
fn sparse_strings_set_grows_past_gaps_and_returns_value_ids() {
    let mut strings = SparseStrings::new();
    assert_eq!(strings.set(0, Some("first")), 0);
    assert_eq!(strings.set(2, Some("second")), 1);
    assert_eq!(strings.len(), 3);
    assert_eq!(strings.get(0), Some("first"));
    assert_eq!(strings.get(1), None);
    assert_eq!(strings.get(2), Some("second"));
}

fn next_splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[test]
fn statement_id_uuid_round_trips_for_deterministic_uuids() {
    let mut seed = 0x517C_C1B7_2722_0A95_u64;
    let empty_fallbacks = SparseStrings::new();
    for _ in 0..256 {
        let compound_num = u32::try_from(next_splitmix(&mut seed) % 1_000_000 + 1).unwrap_or(1);
        let qid = format!("Q{compound_num}");
        let hi = next_splitmix(&mut seed);
        let lo = next_splitmix(&mut seed);
        let raw_bytes = [hi.to_be_bytes(), lo.to_be_bytes()].concat();
        let bytes: [u8; 16] = raw_bytes
            .try_into()
            .expect("concatenated halves are 16 bytes");
        let hex = format!("{hi:016x}{lo:016x}");
        let uuid_lower = format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        );
        let uuid_upper = uuid_lower.to_ascii_uppercase();
        let expected_lower = format!("{qid}-{uuid_lower}");
        let expected_upper = format!("{qid}-{uuid_upper}");

        for (input, want_upper) in [
            (expected_lower.clone(), false),
            (
                format!("{}{expected_lower}", crate::WIKIDATA_STATEMENT_BASE),
                false,
            ),
            (
                format!("https://www.wikidata.org/entity/statement/{expected_lower}"),
                false,
            ),
            (expected_upper.clone(), true),
            (
                format!("{}{expected_upper}", crate::WIKIDATA_STATEMENT_BASE),
                true,
            ),
        ] {
            let parsed =
                StatementId::parse(statement_id::strip_statement_prefix(&input), compound_num);
            if want_upper {
                assert!(
                    matches!(parsed, statement_id::ParsedStatement::Uuid(b) if b == bytes),
                    "uppercase UUID match failed for {input}"
                );
                assert_eq!(
                    StatementId::Uuid(bytes).text(&qid, &empty_fallbacks),
                    Some(expected_upper.clone()),
                    "StatementId::text uppercase mismatch"
                );
            } else {
                assert!(
                    matches!(parsed, statement_id::ParsedStatement::UuidLower(b) if b == bytes),
                    "lowercase UUID match failed for {input}"
                );
                assert_eq!(
                    StatementId::UuidLower(bytes).text(&qid, &empty_fallbacks),
                    Some(expected_lower.clone()),
                    "StatementId::text lowercase mismatch"
                );
            }
        }

        let mismatched =
            StatementId::parse(&format!("Q{}-{uuid_lower}", compound_num + 1), compound_num);
        assert!(
            matches!(mismatched, statement_id::ParsedStatement::Other),
            "mismatched compound prefix must fall back to Other"
        );
    }
}

#[test]
fn contains_folded_matches_naive_folded_substrings_and_rejects_non_ascii_on_ascii() {
    const ASCII_POOL: &[char] = &['a', 'B', 'c', 'D', '0', '9', '-', ' '];
    const GREEK_POOL: &[char] = &['a', 'B', 'α', 'Β', '₁', '₀', ' '];
    let mut seed = 0x243F_6A88_85A3_08D3_u64;

    for i in 0..256 {
        let hay_len = usize::try_from(next_splitmix(&mut seed) % 14).unwrap_or(0);
        let needle_len = usize::try_from(next_splitmix(&mut seed) % 6).unwrap_or(0);
        let pool = if i % 4 == 0 { GREEK_POOL } else { ASCII_POOL };
        let haystack: String = (0..hay_len)
            .map(|_| {
                let idx = usize::try_from(next_splitmix(&mut seed)).unwrap_or(0) % pool.len();
                pool.get(idx).copied().unwrap_or('a')
            })
            .collect();
        let needle_src: String = (0..needle_len)
            .map(|_| {
                let idx = usize::try_from(next_splitmix(&mut seed)).unwrap_or(0) % GREEK_POOL.len();
                GREEK_POOL.get(idx).copied().unwrap_or('a')
            })
            .collect();

        let folded_hay: String = folded(&haystack).into_iter().collect();
        let folded_needle_chars = folded(&needle_src);
        let folded_needle: String = folded_needle_chars.iter().collect();

        assert_eq!(
            contains_folded(&haystack, &folded_needle_chars),
            folded_hay.contains(&folded_needle),
            "contains_folded disagreed with naive folded substring for hay={haystack:?}, needle={needle_src:?}"
        );
        if haystack.is_ascii() && !folded_needle_chars.iter().all(char::is_ascii) {
            assert!(
                !contains_folded(&haystack, &folded_needle_chars),
                "non-ASCII needle must never match ASCII haystack {haystack:?}"
            );
        }
    }
}

#[test]
fn seed_from_grows_bitmask_when_called_after_pushing_more_entities() {
    let mut prev = ColumnarBuilder::new();
    for i in 1..=70_u32 {
        prev.push(RawRow {
            compound_qid: &format!("Q{i}"),
            name: &format!("Compound {i}"),
            reference_qid: &format!("Q{}", 1000 + i),
            ref_title: Some("Ref"),
            ..RawRow::default()
        });
    }
    let prev_set = prev.build();

    let mut next = ColumnarBuilder::new();
    next.push(RawRow {
        compound_qid: "Q1",
        reference_qid: "Q1001",
        ..RawRow::default()
    });
    next.seed_from(&prev_set);
    // Push >64 additional entities so Bitmask must grow on the second seed_from call.
    for i in 2..=70_u32 {
        next.push(RawRow {
            compound_qid: &format!("Q{i}"),
            reference_qid: &format!("Q{}", 1000 + i),
            ..RawRow::default()
        });
    }
    next.seed_from(&prev_set);
    assert_eq!(next.compound_qids().count(), 0);
    assert_eq!(next.reference_qids().count(), 0);
}

#[test]
fn plan_filter_normalizes_unicode_subscripts_in_formula_filter() {
    let mut builder = ColumnarBuilder::new();
    builder.push(RawRow {
        compound_qid: "Q1",
        formula: Some("C₁₅H₁₀O₇"),
        ..RawRow::default()
    });
    let set = builder.build();
    let plan = set.plan_filter(&crate::FilterSpec {
        formula: "C₁₅H₁₀O₇".into(),
        ..Default::default()
    });
    assert_eq!(plan.surviving_count(&set), 1);
}
