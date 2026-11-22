// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::app::routes::{Route, RouteQuery};
use crate::hooks::use_locale;
use crate::i18n::{TextKey, t};
use dioxus::prelude::*;

#[component]
pub fn NotFoundPage() -> Element {
    let locale = use_locale();
    // A route rather than an href: an anchor is a page load, and a window has
    // no server to answer it.
    let home_target = Route::Landing {
        query: RouteQuery::default(),
        hash: String::new(),
    };
    rsx! {
        section {
            class: "w-full max-w-none px-4 pt-3 sm:px-6 lg:px-8",
            aria_labelledby: "not-found-heading",
            div {
                class: "w-full max-w-3xl rounded-xl border border-shell-border bg-shell-raised p-6 shadow-xs sm:p-8",
                h2 {
                    id: "not-found-heading",
                    class: "text-title font-semibold text-text",
                    "{t(locale, TextKey::PageNotFound)}"
                }
                p {
                    class: "mt-3 text-body leading-relaxed text-muted",
                    "{t(locale, TextKey::PageNotFoundDescription)}"
                }
                Link {
                    to: NavigationTarget::Internal(home_target.navigation_string()),
                    class: "inline-flex min-h-[44px] items-center justify-center gap-2 rounded-xl bg-accent px-4 py-2 text-ui font-semibold text-bg no-underline shadow-xs transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] hover:bg-accent-2 active:bg-accent-2 active:scale-[0.98] focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 cursor-pointer",
                    "{t(locale, TextKey::ReturnHome)}"
                }
            }
        }
    }
}
