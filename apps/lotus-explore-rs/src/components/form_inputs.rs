// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Focused, reusable form input components.

use crate::components::ui::Button;
use crate::hooks::use_locale;
use crate::i18n::{TextKey, t};
use dioxus::prelude::*;

#[component]
pub fn TextInput(
    id: String,
    label: String,
    value: String,
    on_change: EventHandler<String>,
    placeholder: Option<String>,
    hint: Option<String>,
) -> Element {
    let hint_id = if hint.is_some() {
        format!("{id}-hint")
    } else {
        String::new()
    };

    rsx! {
        div { class: "flex flex-col gap-1",
            if !label.is_empty() {
                label {
                    r#for: "{id}",
                    class: "text-body font-semibold text-text",
                    "{label}"
                }
            }

            input {
                id: "{id}",
                name: "{id}",
                r#type: "text",
                autocomplete: "off",
                class: "w-full rounded-xl border border-border bg-surface px-3 py-2 text-body text-text placeholder:text-subtle shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                value: "{value}",
                placeholder: placeholder.unwrap_or_default(),
                aria_describedby: if hint_id.is_empty() { "" } else { "{hint_id}" },
                oninput: move |e| on_change.call(e.value()),
            }

            if let Some(hint_text) = hint {
                p { id: "{hint_id}", class: "text-micro text-subtle", "{hint_text}" }
            }
        }
    }
}

#[component]
pub fn RangeInput(
    label: String,
    min_value: f64,
    max_value: f64,
    on_min_change: EventHandler<f64>,
    on_max_change: EventHandler<f64>,
    min_label: String,
    max_label: String,
) -> Element {
    let parse_f64 = |s: &str| s.parse::<f64>().unwrap_or(0.0);
    let min_id = "range-min-input";
    let max_id = "range-max-input";

    rsx! {
        div { class: "flex flex-col gap-1.5",
            label { class: "text-body font-semibold text-text", "{label}" }

            div { class: "flex items-center gap-2",
                div { class: "flex flex-1 flex-col gap-0.5",
                    label { class: "text-micro font-semibold uppercase tracking-wide text-subtle", r#for: "{min_id}", "{min_label}" }
                    input {
                        id: "{min_id}",
                        name: "{min_id}",
                        r#type: "number",
                        autocomplete: "off",
                        class: "w-full rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        value: "{min_value}",
                        oninput: move |e| on_min_change.call(parse_f64(&e.value())),
                    }
                }
                span { class: "self-end pb-1.5 text-subtle", "–" }
                div { class: "flex flex-1 flex-col gap-0.5",
                    label { class: "text-micro font-semibold uppercase tracking-wide text-subtle", r#for: "{max_id}", "{max_label}" }
                    input {
                        id: "{max_id}",
                        name: "{max_id}",
                        r#type: "number",
                        autocomplete: "off",
                        class: "w-full rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                        value: "{max_value}",
                        oninput: move |e| on_max_change.call(parse_f64(&e.value())),
                    }
                }
            }
        }
    }
}

#[component]
pub fn SearchButton(
    #[props(default = false)] loading: bool,
    #[props(default = false)] is_dirty: bool,
    on_click: EventHandler<()>,
) -> Element {
    let locale = use_locale();

    rsx! {
        Button {
            label: if loading {
                t(locale, TextKey::Searching).to_string()
            } else {
                t(locale, TextKey::Search).to_string()
            },
            loading,
            disabled: loading,
            r#type: "submit",
            class: if is_dirty {
                "w-full inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl border border-border bg-accent text-bg font-semibold shadow-xs ring-2 ring-accent/40 hover:bg-accent-2 active:bg-accent-2 min-h-[40px] gap-2 px-3.5 py-2 text-ui active:scale-[0.98]"
            } else {
                "w-full inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl bg-accent text-bg font-semibold shadow-xs hover:bg-accent-2 active:bg-accent-2 min-h-[40px] gap-2 px-3.5 py-2 text-ui active:scale-[0.98]"
            },
            aria_label: t(locale, TextKey::RunSearch).to_string(),
            onclick: move |_| on_click.call(()),
        }
    }
}
