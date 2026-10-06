// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `button`, in their own file.

use super::{Button, POINTER, with_pointer};
use dioxus::prelude::*;

/// An attribute the caller did not ask for must not appear at all.
///
/// The regression the spread exists to prevent: every optional attribute used
/// to render as `unwrap_or_default()`, so every button carried `aria-label=""`,
/// which is not the absence of a label — it overrides the visible text as the
/// accessible name, so a labelled button was announced as nothing.
#[component]
fn Plain() -> Element {
    rsx! { Button { label: Some("Search".to_string()) } }
}

/// A button that does set them.
#[component]
fn Described() -> Element {
    rsx! {
        Button {
            label: Some("Filters".to_string()),
            title: Some("Filter the results".to_string()),
            aria_label: Some("Filter the results".to_string()),
            aria_pressed: Some("true".to_string()),
        }
    }
}

fn render_of(component: fn() -> Element) -> String {
    let mut dom = VirtualDom::new(component);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn an_unset_optional_attribute_is_absent_rather_than_empty() {
    let html = render_of(Plain);
    for attribute in [
        "aria-label",
        "aria-pressed",
        "aria-expanded",
        "aria-controls",
        "title",
    ] {
        assert!(
            !html.contains(attribute),
            "{attribute} was rendered for a button that did not set it:\n{html}"
        );
    }
    assert!(
        html.contains(">Search<"),
        "and the visible label, which is the accessible name, survives:\n{html}"
    );
}

#[test]
fn a_set_optional_attribute_is_rendered() {
    let html = render_of(Described);
    assert!(
        html.contains(r#"aria-label="Filter the results""#),
        "{html}"
    );
    assert!(html.contains(r#"aria-pressed="true""#), "{html}");
    assert!(html.contains(r#"title="Filter the results""#), "{html}");
}

/// An attribute set to whitespace is the same as not set.
///
/// `title: Some(" ".to_string())` renders a tooltip of nothing on hover and
/// announces as nothing, so it is treated as absent rather than shipped.
#[component]
fn Blank() -> Element {
    rsx! {
        Button {
            label: Some("Search".to_string()),
            aria_label: Some("   ".to_string()),
        }
    }
}

#[test]
fn a_blank_optional_attribute_is_treated_as_absent() {
    let html = render_of(Blank);
    assert!(!html.contains("aria-label"), "{html}");
}

#[test]
fn pointer_is_added_when_the_caller_omits_it() {
    assert_eq!(
        with_pointer("inline-flex rounded-xl bg-accent"),
        format!("inline-flex rounded-xl bg-accent {POINTER}")
    );
}

#[test]
fn pointer_is_not_duplicated() {
    let once = with_pointer("rounded-xl");
    assert_eq!(with_pointer(&once), once);
}

#[test]
fn an_existing_pointer_survives_a_substring_lookalike() {
    // `cursor-pointer-events` is a different utility; only the exact token counts.
    let class = "rounded-xl cursor-pointer-events";
    assert_eq!(with_pointer(class), format!("{class} {POINTER}"));
}
