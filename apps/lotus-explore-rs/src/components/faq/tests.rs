// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `faq`, in their own file.

#![allow(clippy::expect_used, clippy::panic)]

use super::category_anchor;
use crate::i18n::{ENTRIES, FaqCategory};
use std::collections::BTreeSet;

/// The page's furniture must not be written inline.
///
/// An explicit list, not a literal scanner: a scanner was tried and false-positived
/// on class lists, on `//` comments, and on the tests' own assertion messages — three
/// false positives in one run, which is how a guard ends up ignored.
#[test]
fn no_user_visible_text_is_hardcoded_in_the_page() {
    let source = include_str!("../faq.rs");
    // Only the rendered component; the test module below is allowed to hold strings.
    let rendered = source.split("#[cfg(test)]").next().unwrap_or_default();

    for literal in [
        "Frequently asked questions",
        "On this page",
        "What this searches",
    ] {
        assert!(
            !rendered.contains(literal),
            "`{literal}` is written inline; the page's own words belong in \
             `faq_chrome`, which is translated. Every answer on this page was \
             translated while the heading above them was not."
        );
    }

    // And positively: the lookup has to be used, or the assertions above would pass
    // against a page that simply deleted its heading.
    assert!(
        rendered.contains("faq_chrome(locale)"),
        "the page must resolve its furniture through `faq_chrome`"
    );
    for field in [
        "chrome.heading",
        "chrome.intro",
        "chrome.contents_label",
        "chrome.contents_heading",
    ] {
        assert!(
            rendered.contains(field),
            "`{field}` is not rendered, so the lookup is not actually used"
        );
    }
}

/// A `ul` here must not be both flex and bulleted: a marker is painted outside the
/// box it belongs to, so `gap` spaces the items while the dots stay at the padding
/// edge. The pair looks correct wide and collapses narrow — a phone-only failure.
#[test]
fn no_list_combines_a_flex_layout_with_markers() {
    let source = include_str!("../faq.rs");
    let mut checked = 0;

    for (index, line) in source.lines().enumerate() {
        // The `ul` and its class are on one line here, how rsx! formats a
        // single-attribute element. An earlier scanner looked for the class on the
        // next line, matched nothing, and passed only because of `checked > 0`.
        if !line.contains("ul {") {
            continue;
        }
        let Some(class) = line
            .split_once("class: \"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(class, _)| class)
        else {
            continue;
        };
        checked += 1;
        assert!(
            !(class.contains("flex") && class.contains("list-disc")),
            "faq.rs:{}: a flex list with markers puts the dots outside the gap: {class}",
            index + 1
        );
        assert!(
            !class.contains("list-disc") || class.contains("pl-"),
            "faq.rs:{}: a bulleted list needs its indent, or the text sits on the \
             dots: {class}",
            index + 1
        );
    }
    assert!(checked > 0, "the scanner found no lists: it is not looking");
}

#[test]
fn category_anchors_are_unique() {
    let anchors: BTreeSet<&str> = FaqCategory::ALL
        .iter()
        .copied()
        .map(category_anchor)
        .collect();
    assert_eq!(
        anchors.len(),
        FaqCategory::ALL.len(),
        "two categories share an anchor, so one heading is unreachable by link"
    );
}

#[test]
fn no_question_anchor_collides_with_a_category_anchor() {
    for entry in ENTRIES {
        for category in FaqCategory::ALL {
            assert_ne!(
                entry.id,
                category_anchor(category),
                "{} collides with a category anchor",
                entry.id
            );
        }
    }
}

#[test]
fn every_category_actually_has_questions() {
    // An empty category renders a heading with nothing under it: a dead end and a
    // useless contents entry.
    for category in FaqCategory::ALL {
        assert!(
            ENTRIES.iter().any(|e| e.category == category),
            "{category:?} has no questions"
        );
    }
}

#[test]
fn the_anchor_helpers_agree_on_the_same_ids() {
    // Guards the rename that would otherwise only show up as a broken link.
    assert_eq!(category_anchor(FaqCategory::About), "faq-about");
    assert_eq!(category_anchor(FaqCategory::Searching), "faq-searching");
    assert_eq!(category_anchor(FaqCategory::Results), "faq-results");
    assert_eq!(category_anchor(FaqCategory::Export), "faq-export");
}
