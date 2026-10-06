// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `loading`, in their own file.

use super::*;

#[test]
fn the_progress_line_counts_in_the_locales_own_way() {
    // The separator and the plural are the reason this reuses `format_count`
    // and `count_label` rather than formatting a number itself: 12,345 in
    // English and 12.345 in German are the same count written differently,
    // and a hand-rolled `format!` would get one of them wrong.
    assert_eq!(
        rows_so_far_text(Locale::En, 12_345),
        "12,345 Entries so far"
    );
    assert_eq!(
        rows_so_far_text(Locale::De, 12_345),
        "12.345 Einträge bisher erhalten"
    );
}

#[test]
fn a_count_below_a_thousand_is_not_grouped() {
    assert_eq!(rows_so_far_text(Locale::En, 7), "7 Entries so far");
    assert_eq!(rows_so_far_text(Locale::En, 1), "1 Entry so far");
}

/// There is deliberately no percentage, and this is why.
#[test]
fn the_progress_line_never_claims_to_know_the_total() {
    let text = rows_so_far_text(Locale::En, 12_345);
    assert!(
        !text.contains('%') && !text.contains("of"),
        "the endpoint does not say how many rows a query will produce, so any \
         denominator is invented: {text}"
    );
}

#[test]
fn preparing_phase_uses_generic_loading_title() {
    assert_eq!(
        query_phase_text(Locale::En, QueryPhase::PreparingQuery),
        query_phase_text(Locale::En, QueryPhase::Idle)
    );
}
