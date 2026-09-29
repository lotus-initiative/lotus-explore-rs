// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The structured data in the page head.
//!
//! Without this, LOTUS Explorer is a page about a dataset that describes
//! nothing about itself. Google Dataset Search and the Bioschemas validator
//! both read the document, and neither can read a page with no markup -- which
//! is the point of emitting it: a search result set is worth indexing even
//! though the page that produced it is an application rather than an article.
//!
//! The markup is built by `lotus-jsonld` and arrives here already serialized.

use dioxus::prelude::*;

use crate::features::explore::selectors::use_header_meta_snapshot;
use crate::state::use_results_context;

/// Make a JSON string safe to sit inside a `<script>` element.
///
/// This is not optional. HTML parsing ends at the first `</script` it sees,
/// whatever the surrounding JavaScript or JSON thinks, so a taxon called
/// `</script><img onerror=...>` would close the element and inject markup. JSON
/// escaping does not help: `/` is not escaped by `serde_json`, because a slash
/// needs no escape inside a JSON string.
///
/// A `U+2028` or `U+2029` is also replaced: those are line terminators to a
/// JavaScript parser, so leaving them in breaks the block outright.
#[must_use]
pub fn escape_for_script_element(json: &str) -> String {
    json.replace("</", "<\\/")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// The `application/ld+json` block for the current result set.
///
/// Renders nothing until a search has produced metadata: a JSON-LD block
/// describing an empty result set is a claim about data that does not exist,
/// and a consumer cannot tell it apart from a real one.
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

    // Rendered in the tree rather than through `document::Script`. That
    // component installs itself with `use_hook`, which runs once: it would keep
    // showing the first search's markup after the second one ran, which is a
    // claim about data the page no longer has. A JSON-LD block is valid
    // anywhere in the document and consumers read it from the body, so a
    // reactive element in the tree is both correct and updatable.
    rsx! {
        script {
            type: "application/ld+json",
            // Escaped above: `serde_json` does not escape `/`, so a slash in the
            // markup would otherwise be able to close this element.
            dangerous_inner_html: escape_for_script_element(json),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn a_closing_script_tag_cannot_escape_the_element() {
        // The whole point: JSON escaping leaves `</script>` intact, so a
        // taxon name that happens to contain one ends the element early.
        let json = r#"{"name":"</script><img src=x onerror=alert(1)>"}"#;
        let escaped = escape_for_script_element(json);
        assert!(
            !escaped.to_lowercase().contains("</script"),
            "still breakable: {escaped}"
        );
        assert!(escaped.contains("<\\/script"), "{escaped}");
    }

    #[test]
    fn a_closing_tag_is_neutralised_wherever_it_appears() {
        for hostile in [
            r#"{"a":"</script>"}"#,
            r#"{"a":"x</SCRIPT >y"}"#,
            r#"["</script","</script"]"#,
        ] {
            let escaped = escape_for_script_element(hostile).to_lowercase();
            assert!(!escaped.contains("</script"), "{hostile} -> {escaped}");
        }
    }

    #[test]
    fn line_terminators_that_break_javascript_are_escaped() {
        // Valid JSON, invalid JavaScript: U+2028 terminates a line in a JS
        // parser, so a taxon name with one in it would throw rather than parse.
        let escaped = escape_for_script_element("{\"a\":\"x\u{2028}y\"}");
        assert!(!escaped.contains('\u{2028}'), "{escaped}");
        assert!(escaped.contains("\\u2028"), "{escaped}");
        assert!(!escaped.contains('\u{2029}'));
    }

    #[test]
    fn ordinary_json_is_unchanged() {
        // The escaping is narrow on purpose: rewriting more than necessary
        // would change bytes a consumer might be hashing.
        let json = r#"{"@type":"Dataset","name":"LOTUS occurrences — Rosa","n":42}"#;
        assert_eq!(escape_for_script_element(json), json);
    }

    #[test]
    fn escaping_preserves_the_json_after_it_is_unescaped_by_a_parser() {
        // `\/` is a legal JSON escape for `/`, so the escaped form still parses
        // to the original string. If this fails, the escaping corrupted it.
        let original = r#"{"name":"a/b</script>c"}"#;
        let escaped = escape_for_script_element(original);
        let parsed: serde_json::Value =
            serde_json::from_str(&escaped).expect("the escaped form is still valid JSON");
        let back = parsed
            .get("name")
            .and_then(serde_json::Value::as_str)
            .expect("a string");
        assert_eq!(back, "a/b</script>c");
    }
}
