// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Document asset helpers for lotus-explore-rs.

// Dioxus's `asset!` macro resolves to `&[u8]`, which clippy's
// `volatile_composites` flags in every `asset!` call site; the value type is
// fixed by the framework and cannot be made volatile-compatible at the call
// site. The lint is suppressed for the whole module because every flagged
// expression is an `asset!` invocation of exactly this shape.
#![allow(clippy::volatile_composites)]

use dioxus::prelude::*;

/// Build a root-relative URL for a static asset under the served `public/` tree.
#[cfg(target_arch = "wasm32")]
pub fn asset_url(path: &str) -> String {
    let path = path.trim_start_matches('/');
    let base = web_sys::window()
        .and_then(|win| win.document())
        .and_then(|doc| {
            doc.document_element()
                .and_then(|html| html.get_attribute("data-lotus-base-path"))
                .filter(|base| !base.is_empty())
                .or_else(|| {
                    doc.query_selector("script[src*='assets/']")
                        .ok()
                        .flatten()
                        .and_then(|el| el.get_attribute("src"))
                        .and_then(|src| src.find("assets/").map(|pos| src[..pos].to_string()))
                        .filter(|base| !base.is_empty())
                })
        })
        .unwrap_or_else(|| String::from("/"));
    format!("{base}{path}")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn asset_url(_path: &str) -> String {
    String::new()
}

/// Injects the curation bridge JS into the document `<head>`, once. Verified not
/// to duplicate on a full load, on SPA navigation to and away from `/curation`,
/// or on re-render of the page's own inputs. The bridge files still tolerate a
/// second injection, which is browser behaviour this component does not control.
#[component]
pub fn CurationScripts() -> Element {
    rsx! {
        document::Script { src: asset!("/public/assets/js/curation/rdkit-bridge.js"), defer: true, "type": "text/javascript" }
        document::Script { src: asset!("/public/assets/js/curation/citation-bridge.js"), defer: true, "type": "text/javascript" }
    }
}

/// The application stylesheet.
///
/// Compiled by `dx` from `tailwind/styles.css` into
/// `public/assets/lotus-explore.css`, and linked from the component tree rather
/// than from `index.html`.
///
/// That indirection is what makes the desktop window styled. `dx` bundles into a
/// desktop binary only the assets that `asset!` names in Rust, so a stylesheet
/// referenced solely from `index.html` is not in the bundle and the window comes
/// up with no CSS at all. Every design token in the theme -- the palette, the
/// spacing, the dark scheme -- is a custom property in this one file, so losing
/// it loses the entire look, not just the utilities.
///
/// On wasm the `asset!` URL resolves to the same `/assets/lotus-explore.css`
/// that `index.html` already links, and Dioxus deduplicates links by href and
/// rel, so rendering this there does not double-load the sheet. It is rendered
/// on native only regardless: the browser document links the sheet at parse time,
/// before the wasm module has booted, which is what keeps the first paint from
/// flashing an unstyled page.
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
