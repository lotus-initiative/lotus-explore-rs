// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The heading a card or section opens with.

use dioxus::prelude::*;

/// A section heading.
///
/// Exists because the curation page had six of these, and five of them carried
/// no classes at all: only "Curation results" was styled, so on one page the same
/// kind of heading was bold in one place and body text in five others. One
/// component is both the fix and the reason it cannot drift again.
#[derive(Props, Clone, PartialEq, Eq)]
pub struct SectionHeadingProps {
    /// The heading text.
    pub text: String,
    /// Heading level, so a card nested in a section does not repeat its `h2`.
    #[props(default = 3)]
    pub level: u8,
    /// Set on the page's one visually-hidden heading.
    #[props(default = false)]
    pub sr_only: bool,
}

/// # Errors
/// Never; an out-of-range level falls back to `h3`.
#[component]
pub fn SectionHeading(props: SectionHeadingProps) -> Element {
    let class = if props.sr_only {
        "sr-only"
    } else {
        "text-base font-semibold"
    };
    let text = props.text.clone();

    // Built as a plain `match` returning an `Element` rather than a `match`
    // inside `rsx!`, which will not parse an element as an arm body.
    let heading = match props.level {
        1 => rsx! { h1 { class: "{class}", {text} } },
        2 => rsx! { h2 { class: "{class}", {text} } },
        4 => rsx! { h4 { class: "{class}", {text} } },
        5 => rsx! { h5 { class: "{class}", {text} } },
        6 => rsx! { h6 { class: "{class}", {text} } },
        _ => rsx! { h3 { class: "{class}", {text} } },
    };

    rsx! { {heading} }
}
