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

/// Lazily inject the curation bridge JS files into the document `<head>`.
#[component]
pub fn CurationScripts() -> Element {
    rsx! {
        document::Script { src: asset!("/public/assets/js/curation/rdkit-bridge.js"), defer: true, "type": "text/javascript" }
        document::Script { src: asset!("/public/assets/js/curation/citation-bridge.js"), defer: true, "type": "text/javascript" }
    }
}
