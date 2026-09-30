// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Clickable examples for a text field.
//!
//! The pattern this replaces was an inert row of `<span>`s that looked like
//! buttons, plus the examples folded into the field's `placeholder`. Both look like
//! they work, and neither does:
//!
//! - A `<span>` that looks like a chip has an affordance it does not have. It is
//!   not focusable, not announced as actionable, and clicking it does nothing. That
//!   is worse than showing nothing: it advertises a capability the control lacks.
//! - A `placeholder` is not a label and is not a durable instruction. It vanishes
//!   the moment anything is typed, it is the lowest-contrast text on the page, and
//!   screen readers announce it inconsistently or not at all. An example nobody can
//!   read once they start typing is not an example.
//!
//! So: a real group of real buttons under a visible heading, each named for what it
//! does, each putting the cursor in the field it filled.
//!
//! ## The two details that make it work
//!
//! **Focus moves to the field.** A button that fills a field and leaves focus where
//! it was means the user has to Tab back to carry on. Moving focus also tells a
//! screen reader the value that was just set, which is cheaper and more reliable
//! than an `aria-live` region saying the same thing.
//!
//! **`type="button"`, always.** A button in a form with no `type` is
//! `type="submit"`, so every suggestion would run the search. Nobody asked for
//! that, and it makes two suggestions in a row impossible.
//!
//! ## Why it asks rather than writes
//!
//! The field is filled by the owner's own state, not by writing to the DOM. These
//! inputs are controlled: a value assigned to the element is overwritten by the
//! next render, so setting one and hoping would flicker back. The owner applies the
//! value and this moves the cursor, which is the only thing it needs the document
//! for -- and it needs that through `eval` rather than `web-sys`, because the
//! browser and a desktop window have different DOM libraries and this crate is
//! built for both.
//!
//! The `<datalist>` alongside each of these stays. It is the right affordance for
//! someone who knows what they want and is typing; this is the right one for
//! someone who does not. Neither replaces the other, and neither replaces the
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
    values: Vec<String>,
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
                        let value = value.clone();
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
    //! What the example buttons have to be.
    //!
    //! The repo's own rule, from `a11y_smoke`: read what came out, not the text of
    //! a file. A source-text assertion passes when the markup is right *and* when a
    //! `sed` left a name behind in a comment, and it fails when rustfmt wraps a
    //! line. So this renders the component and looks at the HTML.
    //!
    //! Every fact asserted here was a bug, or would have been one:
    //!
    //! - The examples were `<span>`s styled like buttons: an affordance the control
    //!   did not have. Not focusable, not actionable, and clicking did nothing.
    //! - A `<button>` with no `type` inside a form is `type="submit"`, so every
    //!   example would have run the search.
    //! - The group had no name, so a screen reader met two buttons apparently
    //!   floating on the page with nothing tying them to the field they fill.
    //! - Each button was named only its value, which says what it is and not what
    //!   it does.

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
                    values: vec!["Fungi".to_string(), "Plantae".to_string()],
                    heading: TextKey::Examples,
                    onfill: move |_value: String| {},
                }
            }
        }
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
        // The old markup. A chip that cannot be activated is worse than no chip,
        // because it advertises a capability the control does not have.
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
