// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::form_actions::FormAction;
use crate::features::explore::selectors::use_criteria_selector;
use crate::i18n::{TextKey, t};
use crate::models::ElementState;
use crate::state::use_form_criteria_context;
use dioxus::prelude::*;

use super::shared::{FormulaSectionState, parse_u16_input};

/// Element requirement select component (F, Cl, Br, I).
#[component]
fn ElemStateSelect(
    label: &'static str,
    value: ElementState,
    on_change: EventHandler<ElementState>,
) -> Element {
    let locale = crate::hooks::use_locale();
    let select_id = format!("{}-select", label.to_lowercase());

    rsx! {
        div { class: "flex flex-col gap-0.5",
            label { class: "text-micro font-semibold uppercase tracking-wide text-subtle", r#for: "{select_id}", "{label}" }
            select {
                id: "{select_id}",
                name: "{select_id}",
                autocomplete: "off",
                class: "w-full rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                aria_label: "{label} {t(locale, TextKey::ElementRequirement)}",
                value: "{value.as_str()}",
                onchange: move |e| on_change.call(e.value().parse::<ElementState>().unwrap_or_default()),
                option { value: "allowed", "{t(locale, TextKey::ElementStateAllowed)}" }
                option { value: "required", "{t(locale, TextKey::ElementStateRequired)}" }
                option { value: "excluded", "{t(locale, TextKey::ElementStateExcluded)}" }
            }
        }
    }
}

/// Formula element count range pair component (C, H, N, etc.).
#[component]
fn NumPair(
    label: &'static str,
    min_value: u16,
    max_value: u16,
    on_min: EventHandler<u16>,
    on_max: EventHandler<u16>,
) -> Element {
    let locale = crate::hooks::use_locale();
    let min_id = format!("{}-min", label.to_lowercase());
    let max_id = format!("{}-max", label.to_lowercase());

    rsx! {
        div { class: "flex flex-col gap-1.5 rounded-xl border border-shell-border bg-shell-raised p-1.5",
            p { class: "text-micro text-subtle", "{label}" }
            div { class: "formula-minmax-grid",
                div { class: "flex flex-col gap-0.5",
                    label { class: "text-micro font-semibold uppercase tracking-wide text-subtle", r#for: "{min_id}", "{t(locale, TextKey::MinCount)}" }
                    input {
                        r#type: "number",
                        id: "{min_id}",
                        name: "{min_id}",
                        autocomplete: "off",
                        class: "tabular-nums w-full rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        min: "0",
                        max: "10000",
                        aria_label: "{label} {t(locale, TextKey::MinCountAria)}",
                        value: "{min_value}",
                        oninput: move |e| {
                            if let Some(v) = parse_u16_input(&e.value()) {
                                on_min.call(v);
                            }
                        },
                    }
                }
                div { class: "flex flex-col gap-0.5",
                    label { class: "text-micro font-semibold uppercase tracking-wide text-subtle", r#for: "{max_id}", "{t(locale, TextKey::MaxCount)}" }
                    input {
                        r#type: "number",
                        id: "{max_id}",
                        name: "{max_id}",
                        autocomplete: "off",
                        class: "tabular-nums w-full rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        min: "0",
                        max: "10000",
                        aria_label: "{label} {t(locale, TextKey::MaxCountAria)}",
                        value: "{max_value}",
                        oninput: move |e| {
                            if let Some(v) = parse_u16_input(&e.value()) {
                                on_max.call(v);
                            }
                        },
                    }
                }
            }
        }
    }
}

/// Formula filter controls section - reads and writes `FormCriteriaContext`.
#[component]
pub fn FormulaSection() -> Element {
    let locale = crate::hooks::use_locale();
    let ctx = use_form_criteria_context();
    let criteria = use_criteria_selector(ctx.criteria, |c| FormulaSectionState {
        formula_enabled: c.formula_enabled,
        formula_exact: c.formula_exact.clone(),
        c_min: c.c_min,
        c_max: c.c_max,
        h_min: c.h_min,
        h_max: c.h_max,
        n_min: c.n_min,
        n_max: c.n_max,
        o_min: c.o_min,
        o_max: c.o_max,
        p_min: c.p_min,
        p_max: c.p_max,
        s_min: c.s_min,
        s_max: c.s_max,
        f_state: c.f_state,
        cl_state: c.cl_state,
        br_state: c.br_state,
        i_state: c.i_state,
    });
    let criteria = criteria.read();
    let enabled = criteria.formula_enabled;

    rsx! {
        div { class: "flex flex-col gap-1.5 rounded-xl border border-shell-border bg-shell-raised p-1.5",
            label { class: "flex cursor-pointer items-center gap-1.5 text-ui text-muted",
                input {
                    r#type: "checkbox",
                    id: "formula-enabled",
                    name: "formula_enabled",
                    "toolparamdescription": "Whether the formula filter is applied.",
                    autocomplete: "off",
                    class: "accent-accent cursor-pointer",
                    checked: enabled,
                    onchange: move |e| ctx.update(FormAction::FormulaEnabled(e.checked())),
                }
                "{t(locale, TextKey::FormulaFilter)}"
            }

            if enabled {
                div { class: "flex flex-col gap-3",
                    div { class: "flex flex-col gap-1.5",
                        label { class: "text-micro font-semibold uppercase tracking-wide text-subtle", r#for: "formula-exact",
                            "{t(locale, TextKey::ExactFormula)}"
                        }
                        input {
                            id: "formula-exact",
                            name: "formula_exact",
                            "toolparamdescription": "Exact molecular formula filter, e.g. C7H5O5N.",
                            r#type: "text",
                            class: "w-full rounded-xl border border-border bg-surface px-3 py-2 text-body text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                            autocomplete: "off",
                            spellcheck: "false",
                            placeholder: "C15H10O5",
                            value: "{criteria.formula_exact}",
                            oninput: move |e| ctx.update(FormAction::FormulaExact(e.value())),
                        }
                    }

                    div { class: "formula-grid grid grid-cols-2 gap-3 md:grid-cols-4 xl:grid-cols-8",
                        NumPair {
                            label: "C",
                            min_value: criteria.c_min,
                            max_value: criteria.c_max,
                            on_min: move |v| ctx.update(FormAction::CMin(v)),
                            on_max: move |v| ctx.update(FormAction::CMax(v)),
                        }
                        NumPair {
                            label: "H",
                            min_value: criteria.h_min,
                            max_value: criteria.h_max,
                            on_min: move |v| ctx.update(FormAction::HMin(v)),
                            on_max: move |v| ctx.update(FormAction::HMax(v)),
                        }
                        NumPair {
                            label: "N",
                            min_value: criteria.n_min,
                            max_value: criteria.n_max,
                            on_min: move |v| ctx.update(FormAction::NMin(v)),
                            on_max: move |v| ctx.update(FormAction::NMax(v)),
                        }
                        NumPair {
                            label: "O",
                            min_value: criteria.o_min,
                            max_value: criteria.o_max,
                            on_min: move |v| ctx.update(FormAction::OMin(v)),
                            on_max: move |v| ctx.update(FormAction::OMax(v)),
                        }
                        NumPair {
                            label: "P",
                            min_value: criteria.p_min,
                            max_value: criteria.p_max,
                            on_min: move |v| ctx.update(FormAction::PMin(v)),
                            on_max: move |v| ctx.update(FormAction::PMax(v)),
                        }
                        NumPair {
                            label: "S",
                            min_value: criteria.s_min,
                            max_value: criteria.s_max,
                            on_min: move |v| ctx.update(FormAction::SMin(v)),
                            on_max: move |v| ctx.update(FormAction::SMax(v)),
                        }
                    }
                    div { class: "formula-grid grid grid-cols-2 gap-3 md:grid-cols-4 xl:grid-cols-8",
                        ElemStateSelect {
                            label: "F",
                            value: criteria.f_state,
                            on_change: move |v| ctx.update(FormAction::FState(v)),
                        }
                        ElemStateSelect {
                            label: "Cl",
                            value: criteria.cl_state,
                            on_change: move |v| ctx.update(FormAction::ClState(v)),
                        }
                        ElemStateSelect {
                            label: "Br",
                            value: criteria.br_state,
                            on_change: move |v| ctx.update(FormAction::BrState(v)),
                        }
                        ElemStateSelect {
                            label: "I",
                            value: criteria.i_state,
                            on_change: move |v| ctx.update(FormAction::IState(v)),
                        }
                    }
                }
            }
        }
    }
}
