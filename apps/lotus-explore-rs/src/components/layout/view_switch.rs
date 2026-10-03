// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! View-switcher nav component (Search / Curation / Structure editor).
//! Reads the active route and `use_locale()` for labels — zero props required.

use crate::app::routes::Route;
#[cfg(target_arch = "wasm32")]
use crate::features::explore::url_state::href_with_current_query;
use crate::hooks::use_locale;
use crate::i18n::faq::faq_nav_label;
use crate::i18n::{
    view_label_curation_explorer, view_label_draw, view_label_explorer, view_switch_aria,
};
use crate::state::use_app_state_context;
use crate::ui::prelude::{SegmentedControl, SegmentedControlItem};
use dioxus::prelude::*;

/// Three-button view switcher.
#[component]
pub fn ViewSwitch() -> Element {
    let ctx = use_app_state_context();
    let locale = use_locale();
    let route: Route = use_route();
    let navigator = dioxus::router::navigator();
    let dark_mode = ctx.state.read().dark_mode;

    rsx! {
        nav { class: "view-switch flex flex-wrap items-center rounded-full overflow-hidden border border-border bg-surface shadow-xs", aria_label: "{view_switch_aria(locale)}",
            SegmentedControl {
                aria_label: "",
                selected_value: route.view_key(),
                dark: dark_mode,
                wrap: true,
                active_aria_current: "page",
                items: vec![
                    SegmentedControlItem {
                        label: view_label_explorer(locale),
                        value: "search",
                    },
                    SegmentedControlItem {
                        label: view_label_curation_explorer(locale),
                        value: "curation",
                    },
                    SegmentedControlItem {
                        label: view_label_draw(locale),
                        value: "draw",
                    },
                    SegmentedControlItem {
                        label: faq_nav_label(locale),
                        value: "faq",
                    },
                ],
                on_select: move |value: String| {
                    let target = route.clone().with_view(&value);
                    if target != route {
                        #[cfg(target_arch = "wasm32")]
                        if route.view_key() == "landing" {
                            let target_path = match value.as_str() {
                                "curation" => "/curation",
                                "draw" => "/draw",
                                "faq" => "/faq",
                                _ => "/search",
                            };
                            if let Some(window) = web_sys::window() {
                                let _ = window.location().set_href(&href_with_current_query(target_path));
                            }
                            return;
                        }
                        let _ = navigator.push(NavigationTarget::Internal(
                            target.navigation_string(),
                        ));
                    }
                },
            }
        }
    }
}
