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

/// No two landmarks may carry the same accessible name.
///
/// Landmarks are navigated by name. Two regions both called "Compound-taxon-
/// reference triples" give a screen reader nothing to tell them apart, which is
/// the `landmark-unique` failure axe reports on the results view.
///
/// Read from source rather than from a rendered page: the pairing that matters is
/// two `rsx!` blocks naming the same string, and no unit test here can render the
/// app. A distinct string per landmark is not enough on its own — the same
/// `TextKey` reached from two places is exactly the mistake this catches.
#[test]
fn no_two_landmarks_share_an_accessible_name() {
    let mut sites: Vec<(String, String)> = Vec::new();
    for dir in ["components", "app", "pages"] {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join(dir);
        collect_landmark_labels(&root, &mut sites);
    }
    // An empty scan would pass every duplicate check there is, so it has to fail
    // here. `>= 1`, not `>= 2`: after the fix there is exactly one named landmark in
    // the results view, and the property under test is "no name is used twice", not
    // "two landmarks exist".
    assert!(
        !sites.is_empty(),
        "the scan found no landmarks at all, so every check below passes vacuously"
    );

    // Compared on the label alone. The location is carried only to name the
    // offender: folding it into the compared string makes every pair unique, which
    // is how the first version of this test passed with the duplicate present.
    let mut seen: HashSet<&String> = HashSet::new();
    let duplicates: Vec<&String> = sites
        .iter()
        .filter(|(label, _)| !seen.insert(label))
        .map(|(_, at)| at)
        .collect();
    assert!(
        duplicates.is_empty(),
        "these landmark names are each used more than once, at: {duplicates:?}"
    );
}

/// Every `role: "region"`/`"main"`/`"banner"`/`"contentinfo"`/`"navigation"`
/// in `rsx!`, with the `aria-label` or `TextKey` on the same element.
fn collect_landmark_labels(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_landmark_labels(&path, out);
            continue;
        }
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            if !LANDMARK_ROLES
                .iter()
                .any(|r| line.contains(&format!("role: \"{r}\"")))
            {
                continue;
            }
            // The label is on this line or the next few: `rsx!` wraps attributes
            // freely and the block that names a landmark spans several lines.
            let window: String = text
                .lines()
                .skip(index)
                .take(4)
                .collect::<Vec<_>>()
                .join(" ");
            let Some(label) = window
                .split("aria_label:")
                .nth(1)
                .and_then(|rest| rest.split("TextKey::").nth(1))
                .and_then(|key| key.split(['}', ',', '"']).next())
                .map(str::trim)
                .filter(|k| !k.is_empty())
            else {
                continue;
            };
            out.push((
                label.to_owned(),
                format!(
                    "{}:{index}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ),
            ));
        }
    }
}

const LANDMARK_ROLES: [&str; 5] = ["region", "main", "banner", "contentinfo", "navigation"];
