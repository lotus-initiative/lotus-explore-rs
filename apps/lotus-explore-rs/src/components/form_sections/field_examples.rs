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
/// `target` is the id of the input to fill and move the cursor into. The group carries
/// a visible heading and the input points at it with `aria-describedby`, so a screen
/// reader hears "Taxon … examples, 6 items" rather than six floating buttons.
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
                                // Not `submit`, which a button in a form defaults to:
                                // every suggestion would run the search.
                                r#type: "button",
                                class: "cursor-pointer rounded-full border border-border bg-panel px-2 py-1 text-micro text-subtle transition-colors hover:border-accent hover:text-text focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                                // The visible text is the bare value, which out of
                                // context does not say what it does; this names the action.
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
/// Through `eval` rather than a DOM crate, because a browser and a desktop window do
/// not share a DOM library. The escaping is a JSON string, so an id with a quote in it
/// cannot end the string early.
pub fn focus(target: &str) {
    let Ok(literal) = serde_json::to_string(target) else {
        return;
    };
    let script = format!("document.getElementById({literal}).focus()");
    // Fire and forget: nothing reads a result, and awaiting makes the click handler
    // wait on a round trip to say nothing.
    let _ = document::eval(&script);
}

#[cfg(test)]
#[path = "field_examples/tests.rs"]
mod tests;
