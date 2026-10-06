// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `document_head`, in their own file.

#![allow(clippy::expect_used, clippy::panic)]

// The panic lints keep shipped code free of panics on external input. A test
// that fails on a missing or stale file is reporting, not panicking.

/// The Tailwind input: the theme as authored.
const THEME_SOURCE: &str = include_str!("../../tailwind/styles.css");
/// The compiled sheet `asset!` embeds and `dx` regenerates.
const COMPILED_SHEET: &str = include_str!("../../public/assets/lotus-explore.css");

/// The custom properties that have to survive compilation.
///
/// Only those declared in `:root` and `[data-theme]`, which are read by the
/// running app. Tokens inside `@theme` are Tailwind's build-time namespace:
/// they become utility classes rather than custom properties, so expecting
/// to find them verbatim in the output would fail on a correct build.
fn runtime_custom_properties() -> Vec<&'static str> {
    let mut block: Option<&str> = None;
    let mut found = Vec::new();
    for line in THEME_SOURCE.lines() {
        let line = line.trim();
        if line.starts_with("@theme") {
            block = Some("theme");
        } else if line.starts_with('@') {
            block = None;
        } else if line.starts_with(":root") || line.starts_with("[data-theme") {
            block = Some("runtime");
        } else if line == "}" {
            block = None;
        } else if block == Some("runtime")
            && let Some((name, _)) = line.split_once(':')
            && name.starts_with("--")
        {
            found.push(name);
        }
    }
    found
}

#[test]
fn the_compiled_sheet_carries_every_theme_token() {
    // The compiled sheet is committed, because `asset!` reads it at compile
    // time and a checkout without it cannot build. That makes it possible
    // for it to go stale: someone edits `tailwind/styles.css`, and nothing
    // rebuilds the sheet until the next `dx build`. A missing token is a
    // silently unstyled component, which is exactly the class of bug that
    // survives review.
    let tokens = runtime_custom_properties();
    assert!(
        !tokens.is_empty(),
        "no theme tokens found in tailwind/styles.css: the block tracking is wrong, \
         and this test would pass on a theme it cannot see"
    );

    let missing: Vec<&&str> = tokens
        .iter()
        .filter(|token| !COMPILED_SHEET.contains(&format!("{token}:")))
        .collect();

    assert!(
        missing.is_empty(),
        "{} theme token(s) are in tailwind/styles.css but not in the committed \
         lotus-explore.css: {missing:?}\nRun `dx build` in apps/lotus-explore-rs \
         and commit the result.",
        missing.len()
    );
}

#[test]
fn the_compiled_sheet_is_not_the_uncompiled_input() {
    // Guards against the committed file being a copy of the source, which
    // would satisfy the token check above while shipping no utilities at all.
    assert!(
        !COMPILED_SHEET.contains("@import \"tailwindcss\""),
        "the committed sheet still contains the Tailwind import, so it is the \
         uncompiled input rather than the build output"
    );
    assert!(
        COMPILED_SHEET.len() > THEME_SOURCE.len(),
        "the compiled sheet ({} bytes) is not larger than its input ({} bytes), \
         so nothing was compiled",
        COMPILED_SHEET.len(),
        THEME_SOURCE.len()
    );
}
