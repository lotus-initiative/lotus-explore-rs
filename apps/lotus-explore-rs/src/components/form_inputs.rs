// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Focused, reusable form input components.

use crate::components::ui::Button;
use crate::hooks::use_locale;
use crate::i18n::{TextKey, t};
use dioxus::prelude::*;

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

/// Clear every search-form filter.
///
/// Secondary styling next to the primary search button, and disabled when there
/// is nothing to clear: a reset that does nothing is a control that lies about
/// what the page can do.
///
/// `type="button"` is load-bearing and not a detail. This sits inside
/// `<form id="lotus-search-form">` beside a `type="submit"` button, and a button
/// with no type in a form is a submit -- so without it, clearing the filters would
/// also run the search that had just been cleared.
#[component]
pub fn ResetFiltersButton(
    #[props(default = false)] disabled: bool,
    on_click: EventHandler<()>,
) -> Element {
    let locale = use_locale();

    rsx! {
        Button {
            r#type: "button",
            disabled,
            // The secondary variant, verbatim as every other secondary button in
            // the app spells it. `w-full` because this sits in a grid cell that
            // sizes to content on wide screens and to the panel on narrow ones.
            class: "w-full inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl border border-border bg-surface text-text font-semibold shadow-xs hover:bg-bg active:bg-bg min-h-[40px] gap-2 px-3.5 py-2 text-ui active:scale-[0.98] disabled:opacity-100 disabled:bg-panel-soft disabled:text-muted disabled:cursor-not-allowed",
            // No `aria_label`: the visible text is the accessible name, and a
            // second name that can drift from it is a WCAG 2.5.3 failure. The
            // label says what it does, which is what a label is for.
            onclick: move |_| on_click.call(()),
            "{t(locale, TextKey::ResetSearchFilters)}"
        }
    }
}
