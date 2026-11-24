// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Loading and download-dispatching overlay components.

use crate::components::ui::button::Button;
use crate::features::explore::interactions::use_explore_interactions;
use crate::features::explore::selectors::use_lifecycle_selector;
use crate::i18n::{CountNoun, Locale, TextKey, t};
use crate::i18n::{count_label, format_count};
use crate::state::use_results_context;
use crate::ui::notice::{NoticeBar, NoticeTone};
use dioxus::prelude::*;
use lotus_explore_core::explore::types::QueryPhase;

/// Branded breathing LOTUS flower indicator.
#[component]
pub fn LotusLoader(#[props(default = "w-14 h-auto")] size_class: &'static str) -> Element {
    rsx! {
        svg {
            xmlns: "http://www.w3.org/2000/svg",
            view_box: "0 -40 1109 750",
            "aria-hidden": "true",
            "focusable": "false",
            class: "lotus-loader {size_class}",
            g { transform: "translate(-725.56083 -269.2746)",
                path {
                    class: "lotus-loader-outer",
                    d: "M1279.8549 270.43606c-39.0859 67.512-64.8441 131.36907-80.5791 191.01307 19.7968 17.25866 38.0323 35.45653 54.7822 54.4924 3.807-7.61414 25.7968-50.43454 25.7969-50.4344 0 0 22.2799 42.82026 26.0868 50.4344 16.4973-19.03534 34.7316-37.23374 54.7823-54.4924-15.9894-59.644-41.7852-123.50107-80.8691-191.01307m-219.9984 117.3904c-4.0609 52.5376-2.3949 100.00467 3.1884 142.8976.2537 0 18.1501 51.95946 70.4343 97.39053 13.4516 11.9288 35.2045 32.9896 52.4633 57.10106 3.0456-10.65986 19.6435-72.36866 59.9996-153.33226-43.4007-51.01466-103.3443-100.65586-186.0856-144.05693m439.9968 0c-82.74 43.40067-142.9387 93.04213-186.0855 144.05693 40.3548 81.21747 56.9548 142.6724 60.0002 153.33226 17.2586-24.11146 39.012-45.17226 52.4626-57.10106 52.284-45.6848 70.4347-97.39053 70.4347-97.39053 5.584-42.89293 7.248-90.36013 3.188-142.8976",
                }
                path {
                    class: "lotus-loader-outer",
                    d: "M1052.0305 546.08619c-26.6495 69.79627-73.42959 248.04093 60.5792 349.56266 28.1723 21.57333 61.5403 20.5568 84.6371 2.02893-21.8272-37.3092-39.835-90.57773-16.2318-191.59279-51.2685-73.6032-89.1366-74.21293-128.9845-159.9988m455.3588 0c-39.5933 86.03973-77.4253 86.3956-128.6947 159.9988 23.604 101.01426 5.596 154.28373-16.232 191.59279 23.3507 18.274 56.4654 19.54414 84.6374-2.02893 133.7546-101.7756 87.192-279.76653 60.2893-349.56266",
                }
                path {
                    class: "lotus-loader-outer",
                    d: "M1725.6493 605.21592c-63.9 2.2248-123.1707 27.53134-173.0427 59.13 6.5987 70.55773-3.7226 154.40853-66.6666 219.1288 7.3613-.25374 14.7413-1.3756 22.6093-2.89854 159.3893-30.20266 101.1-153.79239 282.316-269.27332-22.144-5.01267-43.9173-6.8284-65.216-6.08694m-891.29825.29c-21.29974-.72186-43.07254 1.07374-65.21694 6.0868 181.2164 115.22733 122.92773 239.07133 282.31679 269.27333 7.868 1.52293 15.2477 2.3904 22.6085 2.89853-62.9434-64.97386-73.2649-148.82493-66.6661-219.12879-49.87266-31.78893-109.14306-56.96467-173.04225-59.12987",
                }
                path {
                    class: "lotus-loader-petal lotus-loader-compound",
                    fill: "var(--footer-wd-compound)",
                    d: "M902.17664 469.56513c3.29947 39.08586 6.96707 89.92493 41.7388 143.47719 19.7968 7.86787 45.71 21.14987 65.50676 33.33307 7.6142-64.72013 31.0143-115.65133 31.0143-115.65133-47.71519-20.55813-82.42279-15.98173-138.25986-61.15893",
                }
                path {
                    class: "lotus-loader-petal lotus-loader-reference",
                    fill: "var(--footer-wd-reference)",
                    d: "M1657.8226 469.56513c-55.836 45.1772-90.8333 40.6008-138.5493 61.15893 0 0 23.4 50.9312 31.0147 115.65133 19.796-12.18267 45.7106-25.4652 65.5066-33.33307 34.7707-53.5528 38.7294-104.39106 42.028-143.47719",
                }
                path {
                    class: "lotus-loader-petal lotus-loader-taxon",
                    fill: "var(--footer-wd-taxon)",
                    d: "M1279.8548 499.99953c-123.0952 237.81479-102.9632 340.06305-62.6083 399.70719 31.4717 46.44626 94.0352 46.44706 125.5068 0 40.3547-59.644 60.1973-161.89213-62.8985-399.70719",
                }
            }
        }
    }
}

/// Spinner overlay shown while a query is in-flight.
/// Subscribes to `query_phase` independently so phase-text updates do not
/// propagate to `ResultsViewport` or its siblings.
#[component]
pub fn LoadingState() -> Element {
    let locale = crate::hooks::use_locale::use_locale();
    let explore = use_results_context().explore;
    let query_phase = *use_lifecycle_selector(explore, |lifecycle| lifecycle.query_phase).read();
    // Subscribed to separately from the phase so that a row count ticking up does
    // not re-render everything that reads `query_phase`.
    let rows_so_far = *use_lifecycle_selector(explore, |lifecycle| lifecycle.rows_so_far).read();
    rsx! {
        div {
            role: "status",
            aria_live: "polite",
            aria_busy: "true",
            class: "flex flex-col items-center justify-center gap-3 p-12 text-center text-muted",
            LotusLoader {}
            p { class: "max-w-[28ch] text-body text-text", "{query_phase_text(locale, query_phase)}" }
            // Shown once there is a count to display and kept visible while the
            // dictionary streams join and the table prepares to render.
            if matches!(
                query_phase,
                QueryPhase::FetchingResults
                    | QueryPhase::ProcessingResults
                    | QueryPhase::Rendering
            ) && rows_so_far.is_some()
            {
                p {
                    class: "max-w-[36ch] text-body text-subtle tabular-nums",
                    "{rows_so_far_text(locale, rows_so_far.unwrap_or(0))}",
                }
            } else {
                p { class: "max-w-[36ch] text-body text-subtle", "{t(locale, TextKey::LoadingHint)}" }
            }
        }
    }
}

/// Spinner shown while a download file is being assembled.
#[component]
pub fn DownloadDispatchState() -> Element {
    let locale = crate::hooks::use_locale::use_locale();
    rsx! {
        div {
            role: "status",
            aria_live: "polite",
            aria_busy: "true",
            class: "flex flex-col items-center justify-center gap-3 p-12 text-center text-muted",
            LotusLoader {}
            p { class: "max-w-[28ch] text-body text-text", "{t(locale, TextKey::PreparingDownload)}" }
            p { class: "max-w-[36ch] text-body text-subtle", "{t(locale, TextKey::ExampleApiUrls)}" }
        }
    }
}

/// Notice shown when the URL triggered a download-only mode but the SPARQL
/// query has not materialized yet, offering the user a "Run search" escape.
#[component]
pub fn DownloadOnlyState() -> Element {
    let locale = crate::hooks::use_locale::use_locale();
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
                class: "inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 rounded-xl border border-border bg-surface text-text font-semibold shadow-xs hover:bg-bg active:bg-bg min-h-[44px] min-w-[44px] gap-1.5 px-3.5 py-2 text-ui active:scale-[0.98]",
                onclick: move |_| interactions.preview(),
            }
        }
    }
}

/// The rows received so far, as a line the reader can watch move.
///
/// Reuses [`format_count`] and [`count_label`], which carry the thousands separator
/// and the plural per locale — "12,345 Entries so far", "12.345 Einträge bisher
/// erhalten". The count leads in all four locales, so composing around it is safe.
///
/// **No percentage:** the endpoint does not say how many rows a query will produce,
/// so a denominator would be invented and how much is left is exactly what is
/// unknown.
pub fn rows_so_far_text(locale: Locale, rows: usize) -> String {
    format!(
        "{} {} {}",
        format_count(locale, rows),
        count_label(locale, CountNoun::Entry, rows),
        t(locale, TextKey::LoadingRowsSoFar)
    )
}

/// Maps a `QueryPhase` to the user-facing loading-state label.
pub fn query_phase_text(locale: Locale, phase: QueryPhase) -> &'static str {
    match phase {
        QueryPhase::Idle | QueryPhase::PreparingQuery => t(locale, TextKey::LoadingTitle),
        QueryPhase::ResolvingTaxon => t(locale, TextKey::LoadingResolvingTaxon),
        QueryPhase::ResolvingStructure => t(locale, TextKey::LoadingResolvingStructure),
        QueryPhase::ResolvingReference => t(locale, TextKey::LoadingResolvingReference),
        QueryPhase::FetchingResults => t(locale, TextKey::LoadingFetchingResults),
        QueryPhase::ProcessingResults => t(locale, TextKey::LoadingProcessingResults),
        QueryPhase::Rendering => t(locale, TextKey::LoadingRendering),
    }
}

#[cfg(test)]
#[path = "loading/tests.rs"]
mod tests;
