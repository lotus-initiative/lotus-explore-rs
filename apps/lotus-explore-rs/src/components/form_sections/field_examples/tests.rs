// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Read what came out, not the text of a file — the repo's rule from `a11y_smoke`.
//! A source-text assertion passes when the markup is right *and* when a `sed`
//! left a name in a comment, and fails when rustfmt wraps a line. So render the
//! component and look at the HTML.
//!
//! Every fact asserted here was a bug, or would have been one: examples as inert
//! `<span>` chips; a `<button>` with no `type` inside a form, so each would have
//! run the search; an unnamed group, leaving two buttons floating with nothing
//! tying them to the field they fill; buttons named only their value.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use super::FieldExamples;
use crate::i18n::{Locale, TextKey};
use dioxus::prelude::*;

/// A wrapper supplying the only context `FieldExamples` reads.
#[component]
fn Subject() -> Element {
    use_context_provider(|| Signal::new(Locale::En));
    rsx! {
        div {
            label { r#for: "taxon-input", "Taxon" }
            input { id: "taxon-input", "aria-describedby": "taxon-input-examples-heading" }
            FieldExamples {
                target: "taxon-input",
                values: &["Fungi", "Plantae"],
                heading: TextKey::Examples,
                onfill: move |_value: String| {},
            }
        }
    }
}

/// The reference field's own wiring. The examples were added to the taxon and
/// structure fields first and the reference field carried only a `placeholder`,
/// the affordance the module doc argues against; this pins the wiring so a fourth
/// field cannot arrive with buttons no label points at.
#[component]
fn ReferenceSubject() -> Element {
    use_context_provider(|| Signal::new(Locale::En));
    rsx! {
        div {
            label { r#for: "reference-input", "Reference" }
            input { id: "reference-input", "aria-describedby": "reference-input-examples-heading" }
            FieldExamples {
                target: "reference-input",
                values: &["10.1002/andp.18280880206",
                "10.1021/acs.jnatprod.1C00812",
                "Q28601559",],
                heading: TextKey::Examples,
                onfill: move |_value: String| {},
            }
        }
    }
}

#[test]
fn the_reference_field_points_at_its_own_examples_heading() {
    let mut dom = VirtualDom::new(ReferenceSubject);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);

    assert!(
        html.contains(r#"aria-describedby="reference-input-examples-heading""#),
        "the input must name the heading: {html}"
    );
    assert!(
        html.contains(r#"id="reference-input-examples-heading""#),
        "and the heading must carry that id: {html}"
    );
    assert!(
        html.contains(r#"aria-labelledby="reference-input-examples-heading""#),
        "and the group must be labelled by it: {html}"
    );
    assert!(
        html.contains("10.1021/acs.jnatprod.1C00812"),
        "the first DOI example is the one that has to be there: {html}"
    );
}

fn render() -> String {
    // The tree must be built before the renderer can walk it.
    let mut dom = VirtualDom::new(Subject);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

/// How many times `needle` occurs, so "exactly one" can be asserted.
fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

#[test]
fn the_examples_are_buttons() {
    let html = render();
    assert_eq!(
        count(&html, "<button"),
        2,
        "one button per example:\n{html}"
    );
    // The old markup: a chip that cannot be activated advertises a capability
    // the control does not have.
    assert!(
        !html.contains("<span class=\"rounded-full"),
        "no example is an inert span:\n{html}"
    );
}

#[test]
fn no_example_submits_the_form() {
    let html = render();
    assert_eq!(
        count(&html, r#"type="button""#),
        2,
        "a button in a form with no type is a submit, and every example would \
         run the search:\n{html}"
    );
}

#[test]
fn the_group_is_named_by_a_visible_heading_the_input_points_at() {
    let html = render();
    assert!(
        html.contains(r#"role="group""#),
        "the group is announced as a group:\n{html}"
    );
    assert!(
        html.contains(r#"aria-labelledby="taxon-input-examples-heading""#),
        "and is named by the heading, not by nothing:\n{html}"
    );
    // The id it points at has to exist and has to be the visible heading: a group
    // labelled by an absent element is labelled by nothing.
    assert!(
        html.contains(r#"id="taxon-input-examples-heading""#),
        "the heading carries that id:\n{html}"
    );
}

#[test]
fn each_button_says_what_it_does() {
    let html = render();
    for value in ["Fungi", "Plantae"] {
        let named = format!(r#"aria-label="Set the field to {value}""#);
        assert!(
            html.contains(&named),
            "a button labelled only {value:?} says what it is, not what it \
             does:\n{html}"
        );
    }
}

#[test]
fn the_buttons_are_reachable_by_keyboard() {
    // A real `button` is focusable and activates on Enter and Space, so nothing
    // beyond it being a real button is assertable. This fails first if it is
    // swapped for a div with a click handler.
    let html = render();
    assert!(
        !html.contains(r#"role="button""#),
        "a role is not a button: only the element itself is focusable:\n{html}"
    );
    assert_eq!(count(&html, "<button"), 2, "{html}");
}
