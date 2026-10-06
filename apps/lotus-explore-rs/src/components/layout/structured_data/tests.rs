// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `structured_data`, in their own file.

#![allow(clippy::expect_used, clippy::panic)]

use super::*;

#[test]
fn a_closing_script_tag_cannot_escape_the_element() {
    // JSON escaping leaves `</script>` intact, so a taxon name containing one
    // would end the element early.
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
    // Valid JSON, invalid JavaScript: U+2028 terminates a line in a JS parser,
    // so a taxon name with one in it throws rather than parses.
    let escaped = escape_for_script_element("{\"a\":\"x\u{2028}y\"}");
    assert!(!escaped.contains('\u{2028}'), "{escaped}");
    assert!(escaped.contains("\\u2028"), "{escaped}");
    assert!(!escaped.contains('\u{2029}'));
}

#[test]
fn ordinary_json_is_unchanged() {
    // Narrow on purpose: rewriting more would change bytes a consumer might hash.
    let json = r#"{"@type":"Dataset","name":"LOTUS occurrences — Rosa","n":42}"#;
    assert_eq!(escape_for_script_element(json), json);
}

#[test]
fn escaping_preserves_the_json_after_it_is_unescaped_by_a_parser() {
    // `\/` is a legal JSON escape for `/`, so the escaped form must still parse
    // to the original string.
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
