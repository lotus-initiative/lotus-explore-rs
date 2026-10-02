// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the sort index, in their own file.
//!
//! The cache exists so that re-sorting a large result set does not re-sort it,
//! which means a wrong cache is a table that shows the previous order. Each test
//! therefore compares the cached order against the directly-sorted one.

use super::*;
use lotus_model::ColumnarResultSet;
use std::sync::Arc;

fn entry(
    name: &str,
    mass: Option<f64>,
    formula: Option<&str>,
    taxon_name: &str,
    pub_year: Option<i16>,
    ref_title: Option<&str>,
) -> CompoundEntry {
    CompoundEntry {
        compound_qid: qid_for("Q", name),
        name: Arc::<str>::from(name),
        inchikey: None,
        smiles: None,
        mass,
        formula: formula.map(Arc::<str>::from),
        taxon_qid: qid_for("Q", taxon_name),
        taxon_name: Arc::<str>::from(taxon_name),
        // One reference per row: a title, a DOI and a year belong to the
        // *reference*, so rows sharing one cannot disagree about them.
        reference_qid: qid_for("Q", name),
        ref_title: ref_title.map(Arc::<str>::from),
        ref_doi: None,
        pub_year,
        statement: None,
    }
}

/// A deterministic Wikidata QID for a test's label.
///
/// The store only keeps a cell it recognises as a QID -- `Q-Alpha` is not one,
/// and a row keyed by nothing is dropped rather than shown with a broken link --
/// so a fixture has to use numeric ids. Derived from the label so a test states
/// one thing per row instead of inventing an id to go with it.
fn qid_for(prefix: &str, label: &str) -> Arc<str> {
    let mut n: u32 = 2_166_136_261;
    for byte in label.bytes() {
        n = n.wrapping_mul(16_777_619).wrapping_add(u32::from(byte));
    }
    Arc::<str>::from(format!("{prefix}{}", n % 1_000_000 + 1))
}

#[test]
fn sorts_by_name_ascending_by_default() {
    let rows = vec![
        entry(
            "Gamma",
            Some(3.0),
            Some("C3"),
            "Taxon C",
            Some(2003),
            Some("Ref C"),
        ),
        entry(
            "Alpha",
            Some(1.0),
            Some("C1"),
            "Taxon A",
            Some(2001),
            Some("Ref A"),
        ),
        entry(
            "Beta",
            Some(2.0),
            Some("C2"),
            "Taxon B",
            Some(2002),
            Some("Ref B"),
        ),
    ];

    let order = build_sorted_indices(&rows, SortState::default());
    assert_eq!(order.as_ref(), &[1, 2, 0]);
}

#[test]
fn sorts_by_mass_descending() {
    let rows = vec![
        entry(
            "Alpha",
            Some(10.0),
            Some("C1"),
            "Taxon A",
            Some(2001),
            Some("Ref A"),
        ),
        entry(
            "Beta",
            Some(30.0),
            Some("C2"),
            "Taxon B",
            Some(2002),
            Some("Ref B"),
        ),
        entry(
            "Gamma",
            Some(20.0),
            Some("C3"),
            "Taxon C",
            Some(2003),
            Some("Ref C"),
        ),
    ];

    let order = build_sorted_indices(
        &rows,
        SortState {
            col: SortColumn::Mass,
            dir: SortDir::Desc,
        },
    );
    assert_eq!(order.as_ref(), &[1, 2, 0]);
}

#[test]
fn sorts_optional_reference_titles() {
    let rows = vec![
        entry(
            "Alpha",
            Some(10.0),
            Some("C1"),
            "Taxon A",
            Some(2001),
            Some("Zeta"),
        ),
        entry("Beta", Some(30.0), Some("C2"), "Taxon B", Some(2002), None),
        entry(
            "Gamma",
            Some(20.0),
            Some("C3"),
            "Taxon C",
            Some(2003),
            Some("Alpha"),
        ),
    ];

    let order = build_sorted_indices(
        &rows,
        SortState {
            col: SortColumn::RefTitle,
            dir: SortDir::Asc,
        },
    );
    assert_eq!(order.as_ref(), &[1, 2, 0]);
}

#[test]
fn cache_returns_same_indices_as_direct_sort_for_asc_and_desc() {
    let rows = vec![
        entry(
            "Gamma",
            Some(3.0),
            Some("C3"),
            "Taxon C",
            Some(2003),
            Some("Ref C"),
        ),
        entry(
            "Alpha",
            Some(1.0),
            Some("C1"),
            "Taxon A",
            Some(2001),
            Some("Ref A"),
        ),
        entry(
            "Beta",
            Some(2.0),
            Some("C2"),
            "Taxon B",
            Some(2002),
            Some("Ref B"),
        ),
    ];

    let cache = build_sort_index_cache(Arc::new(ColumnarResultSet::from_entries(&rows)));
    let asc = indices_for_sort(
        &cache,
        SortState {
            col: SortColumn::Name,
            dir: SortDir::Asc,
        },
    );
    let desc = indices_for_sort(
        &cache,
        SortState {
            col: SortColumn::Name,
            dir: SortDir::Desc,
        },
    );

    assert_eq!(
        asc.as_ref(),
        build_sorted_indices(&rows, SortState::default()).as_ref()
    );
    assert_eq!(desc.as_ref(), &[0, 2, 1]);
}

#[test]
fn descending_sort_is_exact_reverse_of_ascending_order() {
    let rows = vec![
        entry(
            "Alpha",
            Some(10.0),
            Some("C1"),
            "Taxon A",
            Some(2001),
            Some("Zeta"),
        ),
        entry(
            "Alpha",
            Some(10.0),
            Some("C1"),
            "Taxon A",
            Some(2001),
            Some("Alpha"),
        ),
        entry(
            "Gamma",
            Some(20.0),
            Some("C3"),
            "Taxon C",
            Some(2003),
            Some("Beta"),
        ),
    ];

    let asc = build_sorted_indices(
        &rows,
        SortState {
            col: SortColumn::RefTitle,
            dir: SortDir::Asc,
        },
    );
    let desc = build_sorted_indices(
        &rows,
        SortState {
            col: SortColumn::RefTitle,
            dir: SortDir::Desc,
        },
    );

    let expected_desc: Vec<u32> = asc.iter().rev().copied().collect();
    assert_eq!(desc.as_ref(), expected_desc.as_slice());
}

#[test]
fn descending_indices_are_cached_per_column() {
    let rows = vec![
        entry(
            "Gamma",
            Some(3.0),
            Some("C3"),
            "Taxon C",
            Some(2003),
            Some("Ref C"),
        ),
        entry(
            "Alpha",
            Some(1.0),
            Some("C1"),
            "Taxon A",
            Some(2001),
            Some("Ref A"),
        ),
        entry(
            "Beta",
            Some(2.0),
            Some("C2"),
            "Taxon B",
            Some(2002),
            Some("Ref B"),
        ),
    ];

    let cache = build_sort_index_cache(Arc::new(ColumnarResultSet::from_entries(&rows)));
    let first = indices_for_sort(
        &cache,
        SortState {
            col: SortColumn::Name,
            dir: SortDir::Desc,
        },
    );
    let second = indices_for_sort(
        &cache,
        SortState {
            col: SortColumn::Name,
            dir: SortDir::Desc,
        },
    );

    assert!(Arc::ptr_eq(&first, &second));
}
