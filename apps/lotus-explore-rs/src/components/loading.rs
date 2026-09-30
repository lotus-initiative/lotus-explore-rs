// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Loading and download-dispatching overlay components.

use crate::components::ui::Button;
use crate::features::explore::interactions::use_explore_interactions;
use crate::features::explore::selectors::use_lifecycle_selector;
use crate::features::explore::types::QueryPhase;
use crate::i18n::{Locale, TextKey, t};
use crate::state::use_results_context;
use crate::ui::prelude::{NoticeBar, NoticeTone};
use dioxus::prelude::*;

/// Spinner overlay shown while a query is in-flight.
/// Subscribes to `query_phase` independently so phase-text updates do not
/// propagate to `ResultsViewport` or its siblings.
#[component]
pub fn LoadingState() -> Element {
    let locale = crate::hooks::use_locale();
    let explore = use_results_context().explore;
    let query_phase = *use_lifecycle_selector(explore, |lifecycle| lifecycle.query_phase).read();
    rsx! {
        div {
            role: "status",
            aria_live: "polite",
            aria_busy: "true",
            class: "flex flex-col items-center justify-center gap-3 p-12 text-center text-muted",
            div { class: "spinner-lg", "aria-hidden": "true" }
            p { class: "max-w-[28ch] text-body text-text", "{query_phase_text(locale, query_phase)}" }
            p { class: "max-w-[36ch] text-ui text-subtle", "{t(locale, TextKey::LoadingHint)}" }
        }
    }
}

/// Spinner shown while a download file is being assembled.
#[component]
pub fn DownloadDispatchState() -> Element {
    let locale = crate::hooks::use_locale();
    rsx! {
        div {
            role: "status",
            aria_live: "polite",
            aria_busy: "true",
            class: "flex flex-col items-center justify-center gap-3 p-12 text-center text-muted",
            div { class: "spinner-lg", "aria-hidden": "true" }
            p { class: "max-w-[28ch] text-body text-text", "{t(locale, TextKey::PreparingDownload)}" }
            p { class: "max-w-[36ch] text-ui text-subtle", "{t(locale, TextKey::ExampleApiUrls)}" }
        }
    }
}

/// Notice shown when the URL triggered a download-only mode but the SPARQL
/// query has not materialized yet, offering the user a "Run search" escape.
#[component]
pub fn DownloadOnlyState() -> Element {
    let locale = crate::hooks::use_locale();
    let interactions = use_explore_interactions();
    rsx! {
        NoticeBar {
            label: t(locale, TextKey::Notice).to_string(),
            tone: NoticeTone::Warning,
            role: "status",
            aria_live: "polite",
            span { class: "notice-value flex-1 min-w-0 text-ui text-muted max-w-[72ch]", "{t(locale, TextKey::ExampleApiUrls)}" }
            Button {
                r#type: "button",
                label: t(locale, TextKey::RunSearch).to_string(),
                class: "inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl border border-border bg-surface text-text font-semibold shadow-xs hover:bg-bg active:bg-bg min-h-[34px] gap-1.5 px-3 py-1.5 text-ui active:scale-[0.98]",
                onclick: move |_| interactions.preview(),
            }
        }
    }
}

/// Maps a `QueryPhase` to the user-facing loading-state label.
pub fn query_phase_text(locale: Locale, phase: QueryPhase) -> &'static str {
    match phase {
        QueryPhase::Idle | QueryPhase::PreparingQuery => t(locale, TextKey::LoadingTitle),
        QueryPhase::ResolvingTaxon => t(locale, TextKey::LoadingResolvingTaxon),
        QueryPhase::FetchingResults => t(locale, TextKey::LoadingFetchingResults),
        QueryPhase::ProcessingResults => t(locale, TextKey::LoadingProcessingResults),
        QueryPhase::Rendering => t(locale, TextKey::LoadingRendering),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparing_phase_uses_generic_loading_title() {
        assert_eq!(
            query_phase_text(Locale::En, QueryPhase::PreparingQuery),
            query_phase_text(Locale::En, QueryPhase::Idle)
        );
    }
}
