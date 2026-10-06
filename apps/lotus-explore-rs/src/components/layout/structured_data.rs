// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The structured data in the page head.
//!
//! Without this, LOTUS Explorer is a page about a dataset that describes nothing
//! about itself. Google Dataset Search and the Bioschemas validator both read the
//! document: a search result set is worth indexing even though the page that
//! produced it is an application rather than an article.
//!
//! Built by `lotus-jsonld` and already serialized when it arrives.

use dioxus::prelude::*;

use crate::features::explore::selectors::use_header_meta_snapshot;
use crate::state::use_results_context;

/// Make a JSON string safe to sit inside a `<script>` element.
///
/// HTML parsing ends at the first `</script` it sees, whatever the surrounding
/// JavaScript or JSON thinks, so a taxon called `</script><img onerror=...>` would
/// close the element and inject markup. JSON escaping does not help: `serde_json`
/// leaves `/` alone because a slash needs no escape inside a JSON string.
///
/// `U+2028` and `U+2029` are line terminators to a JavaScript parser, so leaving
/// them in breaks the block outright.
#[must_use]
pub fn escape_for_script_element(json: &str) -> String {
    json.replace("</", "<\\/")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// The `application/ld+json` block for the current result set.
///
/// Renders nothing until a search has produced metadata: a JSON-LD block describing
/// an empty result set is a claim about data that does not exist, and a consumer
/// cannot tell it apart from a real one.
/// Make an FAQ JSON-LD string safe to sit inside a `<script>` element.
///
/// The same hazard and fix as [`escape_for_script_element`]. The FAQ text is authored
/// rather than derived from a taxon name, so it cannot currently contain a
/// slash-bracket sequence — but that is a property of today's copy, not of the code
/// path, and the next answer mentioning markup would break the page.
#[must_use]
pub fn escape_faq_script_element(json: &str) -> String {
    escape_for_script_element(json)
}

#[component]
pub fn StructuredDataHead() -> Element {
    let explore = use_results_context().explore;
    let header_snapshot = use_header_meta_snapshot(explore);
    let snapshot = header_snapshot.read();

    let Some(json) = snapshot.metadata_json.as_ref() else {
        return rsx! {};
    };
    if json.trim().is_empty() {
        return rsx! {};
    }

    // Rendered in the tree, not through `document::Script`: that component installs
    // itself with `use_hook`, which runs once, so it would keep showing the first
    // search's markup after the second — a claim about data the page no longer has.
    // JSON-LD is valid anywhere and consumers read it from the body.
    rsx! {
        script {
            type: "application/ld+json",
            // `serde_json` does not escape `/`, so a slash in the markup could
            // otherwise close this element.
            dangerous_inner_html: escape_for_script_element(json),
        }
    }
}

#[cfg(test)]
#[path = "structured_data/tests.rs"]
mod tests;
