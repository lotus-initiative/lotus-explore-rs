// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Light / dark theme toggle in the page header.

use crate::app::routes::Route;
#[cfg(target_arch = "wasm32")]
use crate::features::explore::url_state::deployment_href;
use crate::hooks::use_locale;
use crate::i18n::{TextKey, t};
use crate::state::use_app_state_context;
use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[component]
pub fn DarkModeToggle() -> Element {
    let locale = use_locale();
    let ctx = use_app_state_context();
    let mut app_state = ctx.state;
    let dark_mode = app_state.read().dark_mode;
    let route: Route = use_route();
    let navigator = dioxus::router::navigator();
    let label = if dark_mode {
        t(locale, TextKey::DarkMode)
    } else {
        t(locale, TextKey::LightMode)
    };

    rsx! {
        button {
            class: "theme-toggle inline-flex cursor-pointer items-center gap-2 rounded-full border border-border bg-surface px-3 py-2 text-muted shadow-xs hover:border-accent/50 hover:bg-bg hover:text-text focus:outline-none focus-visible:ring-2 focus-visible:ring-accent/40",
            r#type: "button",
            role: "switch",
            "aria-label": t(locale, TextKey::DarkModeToggle),
            "aria-checked": dark_mode.to_string(),
            onclick: move |_| {
                let new_dark_mode = !dark_mode;
                app_state.with_mut(|s| s.dark_mode = new_dark_mode);
                let target = route.clone().with_dark_mode(new_dark_mode);
                #[cfg(target_arch = "wasm32")]
                if route.view_key() == "landing" {
                    let path = if new_dark_mode {
                        "/?dark_mode=true"
                    } else {
                        "/"
                    };
                    if let Some(window) = web_sys::window() {
                        let _ = window.location().set_href(&deployment_href(path));
                    }
                    return;
                }
                let _ = navigator.replace(NavigationTarget::Internal(
                    target.navigation_string(),
                ));

                // Persist to localStorage
                #[cfg(target_arch = "wasm32")]
                {
                    if let Some(win) = web_sys::window()
                        && let Ok(storage) = js_sys::Reflect::get(&win, &"localStorage".into())
                        && !storage.is_undefined()
                        && let Ok(func) = js_sys::Reflect::get(&storage, &"setItem".into())
                        && let Some(set_item) = func.dyn_ref::<js_sys::Function>()
                    {
                        let _ = set_item.call2(&storage, &"dark_mode".into(), &(if new_dark_mode { "true" } else { "false" }).into());
                    }
                }
            },
            span {
                class: "flex h-6 w-6 items-center justify-center rounded-full bg-accent/12 text-accent flex-shrink-0",
                "aria-hidden": "true",
                if dark_mode {
                    // Moon icon
                    svg {
                        class: "w-4 h-4",
                        fill: "none",
                        stroke: "currentColor",
                        view_box: "0 0 24 24",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
                    }
                } else {
                    // Sun icon
                    svg {
                        class: "w-4 h-4",
                        fill: "none",
                        stroke: "currentColor",
                        view_box: "0 0 24 24",
                        stroke_width: "2",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        circle { cx: "12", cy: "12", r: "5" }
                        line { x1: "12", y1: "1", x2: "12", y2: "3" }
                        line { x1: "12", y1: "21", x2: "12", y2: "23" }
                        line { x1: "4.22", y1: "4.22", x2: "5.64", y2: "5.64" }
                        line { x1: "18.36", y1: "18.36", x2: "19.78", y2: "19.78" }
                        line { x1: "1", y1: "12", x2: "3", y2: "12" }
                        line { x1: "21", y1: "12", x2: "23", y2: "12" }
                        line { x1: "4.22", y1: "19.78", x2: "5.64", y2: "18.36" }
                        line { x1: "18.36", y1: "5.64", x2: "19.78", y2: "4.22" }
                    }
                }
            }
            span {
                class: "text-micro font-semibold uppercase tracking-[0.12em] min-w-[4.5ch] text-center",
                "{label}"
            }
        }
    }
}
