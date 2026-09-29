// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Structure depiction cell for results-table rows.
//! Renders a depiction image (lazy-loaded) when available, otherwise a dash.

use crate::components::results_table::row_cells::row_text::RowText;
use crate::i18n::{Locale, aria_chemical_structure};
use dioxus::prelude::*;

/// Truncate alt text to avoid overly long alternative text (>100 chars)
fn truncate_alt(text: &str, max: usize) -> String {
    if text.len() <= max {
        text.to_string()
    } else {
        let mut end = max.saturating_sub(3).min(text.len());
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &text[..end])
    }
}

pub(in crate::components::results_table::row_cells) fn structure_cell(
    locale: Locale,
    _text: RowText,
    depict_url: Option<std::sync::Arc<str>>,
    name: &str,
) -> Element {
    let alt_text = truncate_alt(&aria_chemical_structure(locale, name), 100);
    rsx! {
        td { class: "min-w-0 px-3 py-2.5 align-middle text-ui text-wd-structure shadow-[inset_3px_0_0_var(--footer-wd-structure)]",
            if let Some(url) = depict_url {
                a {
                    href: "{url}",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    img {
                        class: "block aspect-[3/2] h-auto w-full min-w-[120px] bg-transparent object-contain",
                        src: "{url}",
                        alt: "{alt_text}",
                        loading: "lazy",
                        decoding: "async",
                    }
                }
            } else {
                span { class: "text-subtle", "-" }
            }
        }
    }
}
