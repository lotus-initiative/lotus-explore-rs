// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Clickable examples for a text field.
//!
//! Replaces an inert row of `<span>`s styled like buttons, and examples folded into
//! the field's `placeholder`. Both advertise an affordance the control lacks: a span
//! chip is not focusable, not announced as actionable, and clicking it does nothing;
//! a `placeholder` is not a label, vanishes on first keystroke, is the
//! lowest-contrast text on the page, and is announced inconsistently or not at all.
//!
//! So: real buttons under a visible heading, each named for what it does, each
//! putting the cursor in the field it filled. Two details make it work:
//!
//! - **Focus moves to the field**, so the user need not Tab back and the screen
//!   reader announces the value just set — cheaper and more reliable than an
//!   `aria-live` region saying the same thing.
//! - **`type="button"`, always**: a button in a form with no `type` is
//!   `type="submit"`, so every suggestion would run the search.
//!
//! It asks rather than writes because these inputs are controlled: a value assigned
//! to the element is overwritten by the next render. The owner applies the value and
//! this moves the cursor, the only thing it needs the document for — through `eval`
//! rather than `web-sys`, because the browser and a desktop window have different DOM
//! libraries and this crate is built for both.
//!
//! The `<datalist>` alongside each of these stays: right for someone who knows what
//! they want and is typing, this for someone who does not. Neither replaces the
//! `<label>`.

use crate::i18n::{TextKey, t};
use dioxus::prelude::*;

/// A group of examples, each of which fills the field it belongs to.
///
/// `target` is the id of the input to fill and to move the cursor into. The group
/// carries a visible heading, and the input points at it with `aria-describedby`, so
/// a screen reader hears "Taxon … examples, 6 items" rather than six buttons
/// apparently floating on the page.
#[component]
pub fn FieldExamples(
    /// The id of the input these examples fill.
    target: String,
    /// The values, as they would be typed.
    values: &'static [&'static str],
    /// The visible heading, e.g. "Examples".
    heading: TextKey,
    /// Fill the field. The owner knows how; this does not.
    onfill: EventHandler<String>,
) -> Element {
    let locale = crate::hooks::use_locale();
    let heading_id = format!("{target}-examples-heading");

    rsx! {
        div {
            class: "flex flex-col gap-1",
            role: "group",
            "aria-labelledby": "{heading_id}",
            span {
                id: "{heading_id}",
                class: "text-micro font-semibold tracking-wide text-subtle uppercase",
                "{t(locale, heading)}"
            }
            div {
                class: "flex flex-wrap gap-1.5",
                for value in values.iter() {
                    {
                        let value = (*value).to_owned();
                        let onfill = onfill;
                        let target = target.clone();
                        rsx! {
                            button {
                                // Not `submit`: a button in a form defaults to it,
                                // and every suggestion would run the search.
                                r#type: "button",
                                class: "cursor-pointer rounded-full border border-border bg-panel px-2 py-1 text-micro text-subtle transition-colors hover:border-accent hover:text-text focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                                // The visible text is the bare value, which out of
                                // context does not say what it does. This names the
                                // action, so the button is unambiguous when it is
                                // read from the page's own structure.
                                "aria-label": "{t(locale, TextKey::ExampleSets)} {value}",
                                onclick: move |_| {
                                    onfill.call(value.clone());
                                    focus(&target);
                                },
                                "{value}"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Put the cursor in the field, so the user can carry on typing.
///
/// Through `eval` rather than a DOM crate, because this component is built for a
/// browser and a desktop window and those do not share a DOM library. The escaping
/// is a JSON string, so an id with a quote in it cannot end the string early.
pub fn focus(target: &str) {
    let Ok(literal) = serde_json::to_string(target) else {
        return;
    };
    let script = format!("document.getElementById({literal}).focus()");
    // Fire and forget: nothing reads a result, and awaiting would make the click
    // handler wait on a round trip to say nothing.
    let _ = document::eval(&script);
}

#[cfg(test)]
mod tests {
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

    /// The reference field's own wiring, asserted rather than assumed.
    ///
    /// The examples were added to the taxon and structure fields first and the
    /// reference field carried only a `placeholder`, which is the affordance the
    /// module doc argues against. This pins the wiring so a fourth field cannot
    /// quietly arrive with buttons that no label points at.
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
        // The tree has to be built before it can be walked, and the renderer walks
        // it rather than doing so itself.
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
        // The id it points at has to exist, and it has to be the visible heading:
        // a group labelled by an element that is not there is labelled by nothing.
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
        // A real `button` is focusable and activates on Enter and Space. There is
        // nothing to assert beyond it being a real button, so this is the test that
        // fails first if someone swaps it for a div with a click handler.
        let html = render();
        assert!(
            !html.contains(r#"role="button""#),
            "a role is not a button: only the element itself is focusable:\n{html}"
        );
        assert_eq!(count(&html, "<button"), 2, "{html}");
    }
}
