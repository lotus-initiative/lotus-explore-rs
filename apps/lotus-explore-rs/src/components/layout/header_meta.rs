// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Header metadata strip (resolved taxon QID, query/result hashes, total matches).
//!
//! Reads from [`crate::state::ResultsContext`] and `use_locale()` — zero props.

use crate::components::copy_button::CopyButton;
use crate::features::explore::use_header_meta_snapshot;
use crate::i18n::{TextKey, t};
use crate::state::use_form_criteria_context;
use crate::state::use_results_context;
use dioxus::prelude::*;
use std::sync::Arc;

fn hash_prefix(value: &str) -> &str {
    value.get(..12).unwrap_or(value)
}

#[component]
fn ResolvedTaxonMetaItem(locale: crate::i18n::Locale, qid: Arc<str>) -> Element {
    rsx! {
        span { class: "flex items-center gap-1.5 flex-wrap min-w-0",
            span { class: "font-semibold text-text2", "{t(locale, TextKey::ResolvedTaxon)}" }
            span { class: "text-subtle", ":" }
            span { class: "font-mono text-subtle", "{qid}" }
            CopyButton { text: qid, title: t(locale, TextKey::CopyTaxonQid), class: "meta-copy", locale }
        }
    }
}

#[component]
fn QueryHashMetaItem(locale: crate::i18n::Locale, full_hash: Arc<str>) -> Element {
    rsx! {
        span { class: "flex items-center gap-1.5 flex-wrap min-w-0",
            span { class: "font-semibold text-text2", "{t(locale, TextKey::QueryHash)}" }
            span { class: "text-subtle", ":" }
            span { class: "font-mono text-subtle", "{hash_prefix(&full_hash)}" }
            CopyButton { text: full_hash, title: t(locale, TextKey::CopyFullQueryHash), class: "meta-copy", locale }
        }
    }
}

#[component]
fn ResultHashMetaItem(locale: crate::i18n::Locale, full_hash: Arc<str>) -> Element {
    rsx! {
        span { class: "flex items-center gap-1.5 flex-wrap min-w-0",
            span { class: "font-semibold text-text2", "{t(locale, TextKey::ResultHash)}" }
            span { class: "text-subtle", ":" }
            span { class: "font-mono text-subtle", "{hash_prefix(&full_hash)}" }
            CopyButton { text: full_hash, title: t(locale, TextKey::CopyFullResultHash), class: "meta-copy", locale }
        }
    }
}

/// Displays resolved-taxon QID, query/result hashes, and total-match count.
///
/// All items are gathered into a single `page-header-meta` card that only
/// renders when at least one value is present. Each row is a uniform
/// `p.page-meta > span.meta-item` structure.
#[component]
pub fn HeaderMetaSection() -> Element {
    let explore = use_results_context().explore;
    let form_ctx = use_form_criteria_context();
    let locale = crate::hooks::use_locale();
    let header_snapshot = use_header_meta_snapshot(explore);

    let mut criteria_effect_ready = use_signal(|| false);
    let mut meta_visible = use_signal(|| {
        let snapshot = header_snapshot.read();
        snapshot.resolved_qid.is_some()
            || snapshot.query_hash.is_some()
            || snapshot.result_hash.is_some()
    });

    // Criteria changes invalidate the entire metadata strip until fresh results arrive.
    // peek() for the guard so the effect only subscribes to `form_ctx.criteria`, not to itself.
    use_effect(move || {
        let _ = form_ctx.criteria.read();
        if *criteria_effect_ready.peek() {
            meta_visible.set(false);
        } else {
            criteria_effect_ready.set(true);
        }
    });

    // Show metadata again when a fresh metadata tuple is produced.
    // peek() for meta_visible so this effect only subscribes to `header_snapshot`.
    use_effect(move || {
        let current_meta = header_snapshot.read();
        if !*meta_visible.peek() {
            meta_visible.set(
                current_meta.resolved_qid.is_some()
                    || current_meta.query_hash.is_some()
                    || current_meta.result_hash.is_some(),
            );
        }
    });

    let snapshot_ref = header_snapshot.read();
    let resolved_qid_value = snapshot_ref.resolved_qid.clone();
    let query_hash_value = snapshot_ref.query_hash.clone();
    let result_hash_value = snapshot_ref.result_hash.clone();

    let has_meta = *meta_visible.read()
        && (resolved_qid_value.is_some()
            || query_hash_value.is_some()
            || result_hash_value.is_some());

    rsx! {
        if has_meta {
            div { class: "flex flex-wrap items-center gap-2 rounded-xl border border-border bg-panel-soft px-4 py-2.5 text-ui",
                if let Some(qid) = resolved_qid_value.as_ref() {
                    ResolvedTaxonMetaItem { locale, qid: qid.clone() }
                }
                if let Some(qh) = query_hash_value.as_ref() {
                    QueryHashMetaItem { locale, full_hash: qh.clone() }
                }
                if let Some(rh) = result_hash_value.as_ref() {
                    ResultHashMetaItem { locale, full_hash: rh.clone() }
                }
            }
        }
    }
}
