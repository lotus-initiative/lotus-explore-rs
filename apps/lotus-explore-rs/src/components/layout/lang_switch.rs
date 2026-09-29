// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Language-switcher button group.
//! Reads and writes the `Signal<Locale>` from `LocaleProvider` context via
//! [`use_locale_signal`] — zero props required.

use crate::app::routes::Route;
use crate::hooks::{use_locale, use_locale_signal};
use crate::i18n::{Locale, TextKey, t};
use crate::state::use_app_state_context;
use crate::ui::prelude::{SegmentedControl, SegmentedControlItem};
use dioxus::prelude::*;

/// Four-button language switcher (EN / FR / DE / IT).
#[component]
pub fn LangSwitch() -> Element {
    let mut locale_sig = use_locale_signal();
    let locale = use_locale();
    let route: Route = use_route();
    let navigator = dioxus::router::navigator();
    let dark_mode = use_app_state_context().state.read().dark_mode;

    rsx! {
        nav {
            class: "lang-switch inline-flex items-center rounded-full overflow-hidden border border-border bg-surface shadow-xs",
            aria_label: t(locale, TextKey::Language).to_string(),
            SegmentedControl {
                aria_label: t(locale, TextKey::Language),
                selected_value: locale.lang_code(),
                dark: dark_mode,
                wrap: false,
                items: vec![
                    SegmentedControlItem { label: "EN", value: "en" },
                    SegmentedControlItem { label: "FR", value: "fr" },
                    SegmentedControlItem { label: "DE", value: "de" },
                    SegmentedControlItem { label: "IT", value: "it" },
                ],
                on_select: move |value: String| {
                    let next = match value.as_str() {
                        "fr" => Locale::Fr,
                        "de" => Locale::De,
                        "it" => Locale::It,
                        _ => Locale::En,
                    };
                    if *locale_sig.peek() != next {
                        *locale_sig.write() = next;
                        let target = route.clone().with_locale(next);
                        let _ = navigator.replace(NavigationTarget::Internal(
                            target.navigation_string(),
                        ));
                    }
                },
            }
        }
    }
}
