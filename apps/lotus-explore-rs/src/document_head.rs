// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Document asset helpers for lotus-explore-rs.

// Dioxus's `asset!` macro resolves to `&[u8]`, which clippy's
// `volatile_composites` flags in every `asset!` call site; the value type is
// fixed by the framework and cannot be made volatile-compatible at the call
// site. The lint is suppressed for the whole module because every flagged
// expression is an `asset!` invocation of exactly this shape.
#![allow(
    clippy::volatile_composites,
    reason = "expands from Dioxus's `asset!` macro, which reads bytes through a volatile pointer; the shape is fixed by the macro"
)]

use dioxus::prelude::*;

/// Injects the curation page's third-party JS into the document `<head>`.
///
/// The `RDKit` loader is tagged here rather than by the bridge. The bridge used
/// to build `assets/vendor/rdkit/RDKit_minimal.js` in JS, which is a path the
/// bundler cannot see: it embeds only what `asset!` names, so the file was
/// absent from a desktop bundle and the request 404ed. A path assembled at
/// runtime is invisible to `dx` by construction.
///
/// The wasm module needs a `locateFile` override, and that is the one URL that
/// cannot be a `src` -- see [`RDKIT_WASM_GLOBAL`].
///
/// The loader is omitted when `RDKit` was never fetched. It is 7 MB of wasm that
/// `fetch-assets` downloads, and a build that skipped it should not leave a tag
/// pointing at a file the bundler never had. The bridge already reports a missing
/// toolkit, so the page degrades to a clear error rather than a silent hang.
#[component]
pub fn CurationScripts() -> Element {
    let loader = crate::vendor_assets::rdkit_script_url();
    let wasm = crate::vendor_assets::rdkit_wasm_url();

    // `document::eval` rather than an inline `<script>`. Dioxus renders a
    // `document::Script` from a single text node and rejects anything else, and
    // the text would have to be HTML-escaped, which a script body does not
    // unescape. Ordering is not a concern: this runs during the render, and the
    // loader below is `defer`red, so it executes after.
    use_effect(move || {
        if let Some(wasm) = wasm.as_deref() {
            document::eval(&format!("window.{RDKIT_WASM_GLOBAL} = '{wasm}';"));
        }
    });

    rsx! {
        document::Script { src: asset!("/public/assets/js/curation/rdkit-bridge.js"), defer: true, "type": "text/javascript" }
        if let Some(loader) = loader {
            document::Script { src: "{loader}", defer: true, "type": "text/javascript" }
        }
        document::Script { src: asset!("/public/assets/js/curation/citation-bridge.js"), defer: true, "type": "text/javascript" }
    }
}

/// The global the `RDKit` bridge reads the wasm module's URL from.
///
/// A `locateFile` callback, not a `src`, because the loader looks the module up
/// by name relative to itself. The bundled copy is content-addressed, so that
/// name is not what the loader asks for.
const RDKIT_WASM_GLOBAL: &str = "__lotusRDKitWasm";

/// The application stylesheet, on native only.
///
/// `dx` embeds only the assets Rust names in an `asset!` call, so a stylesheet
/// linked only from `index.html` is absent from a desktop bundle and the window
/// comes up unstyled. The browser document keeps that link, because it loads at
/// parse time and the wasm module has not booted yet; rendering this on wasm too
/// would only add a duplicate that Dioxus then deduplicates.
#[component]
pub fn AppStylesheet() -> Element {
    rsx! {
        document::Stylesheet { href: asset!("/public/assets/lotus-explore.css") }
    }
}

#[cfg(test)]
mod tests {
    // The panic lints keep shipped code free of panics on external input. A test
    // that fails on a missing or stale file is reporting, not panicking.
    #![allow(clippy::expect_used, clippy::panic)]

    /// The Tailwind input: the theme as authored.
    const THEME_SOURCE: &str = include_str!("../tailwind/styles.css");
    /// The compiled sheet `asset!` embeds and `dx` regenerates.
    const COMPILED_SHEET: &str = include_str!("../public/assets/lotus-explore.css");

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
}
