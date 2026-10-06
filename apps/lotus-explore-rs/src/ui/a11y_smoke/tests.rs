// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `a11y_smoke`, in their own file.

use super::super::a11y_contract::{
    MAIN_PANEL_ID, PAGE_TITLE_ID, RESULTS_SECTION_HEADING_ID, RESULTS_SECTION_ID,
    SEARCH_PANEL_BODY_ID, SKIP_TO_RESULTS_HREF,
};
use std::collections::HashSet;

/// Every ID the app hands to assistive technology.
fn all_ids() -> [&'static str; 5] {
    [
        MAIN_PANEL_ID,
        PAGE_TITLE_ID,
        SEARCH_PANEL_BODY_ID,
        RESULTS_SECTION_ID,
        RESULTS_SECTION_HEADING_ID,
    ]
}

#[test]
fn ids_are_unique_and_non_empty() {
    for id in all_ids() {
        assert!(!id.trim().is_empty(), "an empty id is not an id");
        assert!(
            !id.contains(char::is_whitespace),
            "'{id}' contains whitespace, which is not a valid fragment"
        );
    }
    let unique: HashSet<&str> = all_ids().into_iter().collect();
    assert_eq!(
        unique.len(),
        all_ids().len(),
        "two landmarks share an id, so one of them is unreachable"
    );
}

#[test]
fn the_skip_link_resolves_to_the_main_landmark() {
    // The one relationship that is genuinely derivable here: a fragment that
    // does not name an existing element is a skip link that skips nothing.
    assert!(SKIP_TO_RESULTS_HREF.starts_with('#'));
    let target = SKIP_TO_RESULTS_HREF.trim_start_matches('#');
    assert_eq!(target, MAIN_PANEL_ID, "the skip link points at nothing");
}

#[test]
fn ids_are_css_selector_safe() {
    // These values end up in `href="#…"` and in query selectors, so a digit
    // first or a stray dot would silently break one of the two.
    for id in all_ids() {
        assert!(
            id.chars().next().is_some_and(|c| c.is_ascii_alphabetic()),
            "'{id}' should start with a letter, to be a valid CSS selector"
        );
        assert!(
            id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "'{id}' contains a character that is not safe in a selector"
        );
    }
}
