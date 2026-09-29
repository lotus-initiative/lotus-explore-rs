// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::form_actions::FormAction;
use crate::features::explore::interactions::use_explore_interactions;
use crate::features::explore::selectors::use_criteria_selector;
use crate::i18n::{TextKey, t};
use crate::state::use_form_criteria_context;
use dioxus::prelude::*;

use super::shared::{normalized_year_input_max, parse_f64_input, parse_u16_input};

pub(super) const TAXON_SUGGESTIONS: &[&str] = &["Fungi", "Bacteria", "Plantae", "Animalia", "*"];

#[component]
pub fn TaxonInput() -> Element {
    let locale = crate::hooks::use_locale();
    let ctx = use_form_criteria_context();
    let interactions = use_explore_interactions();
    let taxon = use_criteria_selector(ctx.criteria, |c| c.taxon.clone());
    rsx! {
        div { class: "flex flex-col gap-1.5 rounded-xl border border-shell-border bg-shell-raised p-1.5",
            label {
                class: "text-body font-semibold text-text",
                r#for: "taxon-input",
                "{t(locale, TextKey::Taxon)}"
            }
            input {
                id: "taxon-input",
                name: "taxon",
                "toolparamdescription": "Taxon name, Wikidata QID, or * for all taxa.",
                r#type: "text",
                autocomplete: "off",
                spellcheck: "false",
                placeholder: "{t(locale, TextKey::TaxonPlaceholder)}",
                value: "{taxon.read()}",
                class: "w-full rounded-xl border border-border bg-surface px-3 py-2 text-body text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                list: "taxon-suggestions",
                oninput: move |e| ctx.update(FormAction::Taxon(e.value())),
                onkeydown: move |e| {
                    if e.key() == Key::Enter {
                        interactions.search();
                    }
                },
            }
            datalist { id: "taxon-suggestions",
                for item in TAXON_SUGGESTIONS {
                    option { value: "{item}" }
                }
            }
            div { class: "flex flex-wrap gap-1.5",
                for item in TAXON_SUGGESTIONS {
                    span {
                        class: "rounded-full border border-border bg-panel px-2 py-1 text-micro text-subtle",
                        "{item}"
                    }
                }
            }
        }
    }
}

#[component]
pub fn MassRangeInput() -> Element {
    let locale = crate::hooks::use_locale();
    let ctx = use_form_criteria_context();
    let mass_range = use_criteria_selector(ctx.criteria, |c| (c.mass_min, c.mass_max));
    let (min_value, max_value) = *mass_range.read();

    rsx! {
        div {
            role: "group",
            aria_labelledby: "mass-range-label",
            class: "flex flex-col gap-1.5 rounded-xl border border-shell-border bg-shell-raised p-1.5",
            p { id: "mass-range-label", class: "text-body font-semibold text-text", "{t(locale, TextKey::MolecularMass)}" }
            div { class: "grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-end gap-2",
                div { class: "flex min-w-0 flex-col gap-0.5",
                    label {
                        class: "text-micro font-semibold uppercase tracking-wide text-subtle",
                        r#for: "mass-min",
                        "{t(locale, TextKey::Min)}"
                    }
                    input {
                        id: "mass-min",
                        name: "mass_min",
                        "toolparamdescription": "Minimum molecular mass in Da.",
                        r#type: "number",
                         autocomplete: "off",
                        min: "0",
                        max: "10000",
                        step: "1",
                        value: "{min_value}",
                        class: "w-full rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        oninput: move |e| {
                            if let Some(v) = parse_f64_input(&e.value()) {
                                ctx.update(FormAction::MassMin(v));
                            }
                        },
                    }
                }
                span { aria_hidden: "true", class: "pb-2 text-subtle", "-" }
                div { class: "flex min-w-0 flex-col gap-0.5",
                    label {
                        class: "text-micro font-semibold uppercase tracking-wide text-subtle",
                        r#for: "mass-max",
                        "{t(locale, TextKey::Max)}"
                    }
                    input {
                        id: "mass-max",
                        name: "mass_max",
                        "toolparamdescription": "Maximum molecular mass in Da.",
                        r#type: "number",
                        autocomplete: "off",
                        min: "0",
                        max: "10000",
                        step: "1",
                        value: "{max_value}",
                        class: "w-full rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        oninput: move |e| {
                            if let Some(v) = parse_f64_input(&e.value()) {
                                ctx.update(FormAction::MassMax(v));
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
pub fn YearRangeInput() -> Element {
    let locale = crate::hooks::use_locale();
    let ctx = use_form_criteria_context();
    let year_range = use_criteria_selector(ctx.criteria, |c| (c.year_min, c.year_max));
    let (min_value, max_value) = *year_range.read();
    let current = normalized_year_input_max(crate::clock::current_year());

    rsx! {
        div {
            role: "group",
            aria_labelledby: "year-range-label",
            class: "flex flex-col gap-1.5 rounded-xl border border-shell-border bg-shell-raised p-1.5",
            p { id: "year-range-label", class: "text-body font-semibold text-text", "{t(locale, TextKey::PublicationYear)}" }
            div { class: "grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-end gap-2",
                div { class: "flex min-w-0 flex-col gap-0.5",
                    label {
                        class: "text-micro font-semibold uppercase tracking-wide text-subtle",
                        r#for: "year-min",
                        "{t(locale, TextKey::YearFrom)}"
                    }
                    input {
                        id: "year-min",
                        name: "year_min",
                        "toolparamdescription": "Minimum publication year.",
                        r#type: "number",
                        autocomplete: "off",
                        min: "{lotus_model::YEAR_MIN}",
                        max: "{current}",
                        step: "1",
                        value: "{min_value}",
                        class: "w-full rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        oninput: move |e| {
                            if let Some(v) = parse_u16_input(&e.value()) {
                                ctx.update(FormAction::YearMin(v));
                            }
                        },
                    }
                }
                span { aria_hidden: "true", class: "pb-2 text-subtle", "-" }
                div { class: "flex min-w-0 flex-col gap-0.5",
                    label {
                        class: "text-micro font-semibold uppercase tracking-wide text-subtle",
                        r#for: "year-max",
                        "{t(locale, TextKey::YearTo)}"
                    }
                    input {
                        id: "year-max",
                        name: "year_max",
                        "toolparamdescription": "Maximum publication year.",
                        r#type: "number",
                        autocomplete: "off",
                        min: "{lotus_model::YEAR_MIN}",
                        max: "{current}",
                        step: "1",
                        value: "{max_value}",
                        class: "w-full rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        oninput: move |e| {
                            if let Some(v) = parse_u16_input(&e.value()) {
                                ctx.update(FormAction::YearMax(v));
                            }
                        },
                    }
                }
            }
        }
    }
}
