// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Cold-start invitation for bare `/network` before any search has run.
//!
//! Respects `docs/QLEVER-POLITENESS.md` by never auto-firing an endpoint query on
//! load: the user picks one of four curated starter cards, submits a quick entity
//! lookup, or opens `/search` for full filters.

use super::presentation::{EntityShapeIcon, entity_badge_classes};
use crate::app::routes::Route;
use crate::components::ui::Button;
use crate::i18n::{Locale, TextKey, t};
use crate::state::use_app_state_context;
use crate::ui::prelude::{SegmentedControl, SegmentedControlItem};
use dioxus::prelude::*;
use lotus_explore_core::graph::{NETWORK_STARTERS, NetworkStarter, NodeKind};

const QUICK_SEARCH_INPUT_ID: &str = "network-quick-search-input";

/// Build [`lotus_model::SearchCriteria`] for the quick-search box on `/network`.
///
/// Returns `None` for empty/whitespace-only queries.
pub fn quick_search_criteria(
    kind: NodeKind,
    raw: &str,
    year_max: u16,
) -> Option<lotus_model::SearchCriteria> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut criteria = lotus_model::SearchCriteria::up_to_year(year_max);
    match kind {
        NodeKind::Taxon => {
            trimmed.clone_into(&mut criteria.taxon);
        }
        NodeKind::Compound => {
            trimmed.clone_into(&mut criteria.structure);
            "*".clone_into(&mut criteria.taxon);
        }
        NodeKind::Reference => {
            trimmed.clone_into(&mut criteria.reference);
        }
    }
    Some(criteria)
}

fn starter_copy_keys(starter: &NetworkStarter) -> (TextKey, TextKey) {
    match starter.id {
        "beta_sitosterol" => (
            TextKey::NetworkStarterTitleBetaSitosterol,
            TextKey::NetworkStarterDescBetaSitosterol,
        ),
        "gentiana_genus" => (
            TextKey::NetworkStarterTitleGentianaGenus,
            TextKey::NetworkStarterDescGentianaGenus,
        ),
        "gentiana_lutea" => (
            TextKey::NetworkStarterTitleGentianaLutea,
            TextKey::NetworkStarterDescGentianaLutea,
        ),
        _ => (
            TextKey::NetworkStarterTitleIndole,
            TextKey::NetworkStarterDescIndole,
        ),
    }
}

/// Interactive starter workspace rendered when no result set has been loaded yet.
#[component]
pub(super) fn NetworkEmptyStarter(on_run: EventHandler<lotus_model::SearchCriteria>) -> Element {
    let locale = crate::hooks::use_locale();
    let dark_mode = use_app_state_context().state.read().dark_mode;
    let mut selected_kind = use_signal(|| NodeKind::Taxon);
    let mut query_text = use_signal(String::new);

    let can_submit = !query_text.read().trim().is_empty();

    rsx! {
        div { class: "mx-auto flex w-full max-w-4xl flex-col gap-6 py-4",
            div { class: "flex flex-col gap-1.5 text-center sm:text-left",
                h3 { class: "m-0 font-semibold text-title text-text",
                    "{t(locale, TextKey::NetworkStarterHeading)}"
                }
                p { class: "m-0 max-w-[68ch] text-body leading-relaxed text-muted",
                    "{t(locale, TextKey::NetworkStarterSubtitle)}"
                }
            }

            form {
                class: "flex flex-col gap-3 rounded-xl border border-panel-border bg-panel p-4 shadow-xs",
                onsubmit: move |evt| {
                    evt.prevent_default();
                    let kind = *selected_kind.read();
                    let raw = query_text.read().clone();
                    if let Some(criteria) =
                        quick_search_criteria(kind, &raw, crate::clock::current_year())
                    {
                        on_run.call(criteria);
                    }
                },
                div { class: "flex flex-wrap items-center justify-between gap-2",
                    label {
                        r#for: QUICK_SEARCH_INPUT_ID,
                        class: "text-ui font-semibold text-text",
                        "{t(locale, TextKey::NetworkQuickSearchLabel)}"
                    }
                    div { class: "min-w-0 max-w-full",
                        SegmentedControl {
                            aria_label: t(locale, TextKey::NetworkQuickSearchKindLabel),
                            selected_value: selected_kind.read().as_str(),
                            dark: dark_mode,
                            wrap: true,
                            items: vec![
                                SegmentedControlItem {
                                    label: t(locale, TextKey::Taxon),
                                    value: "taxon",
                                },
                                SegmentedControlItem {
                                    label: t(locale, TextKey::Compound),
                                    value: "compound",
                                },
                                SegmentedControlItem {
                                    label: t(locale, TextKey::Reference),
                                    value: "reference",
                                },
                            ],
                            on_select: move |value: String| {
                                selected_kind.set(NodeKind::parse(&value).unwrap_or(NodeKind::Taxon));
                            },
                        }
                    }
                }
                div { class: "flex flex-col gap-2 sm:flex-row sm:items-center",
                    input {
                        id: QUICK_SEARCH_INPUT_ID,
                        r#type: "search",
                        value: "{query_text}",
                        placeholder: "{t(locale, TextKey::NetworkQuickSearchPlaceholder)}",
                        class: "min-h-[44px] min-w-0 flex-1 rounded-xl border border-border bg-surface px-3 py-2 text-body text-text shadow-xs placeholder:text-subtle focus-visible:border-accent focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                        oninput: move |evt| query_text.set(evt.value()),
                    }
                    Button {
                        r#type: "submit",
                        disabled: !can_submit,
                        label: t(locale, TextKey::NetworkQuickSearchSubmit).to_string(),
                        class: "inline-flex shrink-0 items-center justify-center rounded-xl bg-primary px-4 py-2 text-body font-semibold text-white shadow-xs transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] hover:bg-primary-hover focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 disabled:cursor-not-allowed disabled:opacity-55 min-h-[44px]",
                    }
                }
            }

            div { class: "grid grid-cols-1 gap-3 sm:grid-cols-2",
                for starter in NETWORK_STARTERS {
                    StarterCard { starter, locale, on_run }
                }
            }

            div { class: "flex flex-wrap items-center justify-center sm:justify-start",
                Button {
                    r#type: "button",
                    label: t(locale, TextKey::OpenAdvancedSearch).to_string(),
                    class: "inline-flex items-center justify-center rounded-xl border border-border bg-surface px-3.5 py-2 text-ui font-semibold text-text shadow-xs hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 min-h-[44px]",
                    onclick: move |_| Route::navigate_to_view("search"),
                }
            }
        }
    }
}

#[component]
fn StarterCard(
    starter: NetworkStarter,
    locale: Locale,
    on_run: EventHandler<lotus_model::SearchCriteria>,
) -> Element {
    let (title_key, desc_key) = starter_copy_keys(&starter);
    let badge_class = entity_badge_classes(starter.kind);
    let title = t(locale, title_key);
    let desc = t(locale, desc_key);

    rsx! {
        button {
            r#type: "button",
            class: "group flex w-full cursor-pointer flex-col items-start gap-2 rounded-xl border border-panel-border bg-panel p-4 text-left shadow-xs transition-colors duration-150 hover:border-border hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
            onclick: move |_| {
                on_run.call(starter.to_criteria(crate::clock::current_year()));
            },
            div { class: "flex w-full items-center justify-between gap-2",
                span { class: "font-semibold text-body text-text group-hover:underline",
                    "{title}"
                }
                span { class: "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-micro font-semibold {badge_class}",
                    EntityShapeIcon { kind: starter.kind }
                }
            }
            p { class: "m-0 text-body leading-relaxed text-muted", "{desc}" }
        }
    }
}
