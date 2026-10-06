// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

/// The wasm-only module, as text. Compiled here only to be read.
const SOURCE: &str = include_str!("file_sink.rs");

#[test]
fn availability_is_read_from_navigator_not_window() {
    assert!(
        !SOURCE.contains(r#"Reflect::get(&window, &"storage""#),
        "there is no `window.storage`; the origin private file system is \
         `navigator.storage`, so this reads a property that does not exist"
    );
    assert!(
        SOURCE.contains("navigator().storage()"),
        "OPFS availability should be read from `navigator().storage()`"
    );
}

#[test]
fn presence_is_not_tested_by_asking_whether_fetching_threw() {
    // `Reflect::get` reports a missing property as `Ok(undefined)`, so
    // `.is_ok()` on its result is true for a browser that has never heard of
    // OPFS. Both the check and the lookup have to test for a function.
    assert!(
        !SOURCE.contains(r#""getDirectory".into()).is_ok()"#),
        "`Reflect::get` returns Ok for a missing property, so this claims \
         OPFS exists wherever it does not"
    );
    // Counted over code lines, and deliberately not by naming the two
    // expressions. Naming them was the previous spelling and it broke on a
    // `clippy::redundant_closure` fix that changed one of them from
    // `|value| value.is_function()` to `JsValue::is_function` without
    // changing what the code means -- a source gate that fails when a lint
    // rewrites a closure is a gate that gets deleted rather than fixed.
    //
    // Comments are excluded because the module's own doc explains why
    // `is_function` is there, and a count that includes the explanation
    // fails whenever the explanation is reworded.
    let code: String = SOURCE
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let checks = code.matches("is_function").count();
    assert!(
        checks >= 2,
        "both the availability check and the directory lookup have to test \
         for a function rather than for the absence of an error; found \
         {checks} in code"
    );
}
