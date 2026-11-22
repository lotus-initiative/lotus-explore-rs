// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Page header: title, language switcher, view switcher, subtitle, archive note.
//! Zero props -- all data comes from context (`use_locale`, `AppStateContext`).

use crate::app::routes::Route;
use crate::components::layout::dark_mode_toggle::DarkModeToggle;
use crate::components::layout::lang_switch::LangSwitch;
use crate::components::layout::view_switch::ViewSwitch;
use crate::hooks::use_locale;
use crate::i18n::{TextKey, t};
use crate::ui::a11y_contract::PAGE_TITLE_ID;
use dioxus::prelude::*;

const LOTUS_LOGO_SVG: &str = include_str!("../../../public/favicon.svg");

/// Full page header section.
#[component]
pub fn PageHeader() -> Element {
    let locale = use_locale();
    let route: Route = use_route();
    let home = route.with_view("landing");

    rsx! {
        header {
             // Phones get a static header: at 390px the wrapped nav plus subtitle
             // would otherwise keep over 40% of the viewport sticky for the whole session.
             class: "z-3 bg-shell-chrome px-4 py-3 sm:sticky sm:top-0 sm:mx-2 sm:mt-2 sm:rounded-t-xl sm:px-8 sm:py-3.5",

            div {
                class: "flex flex-col gap-3 sm:flex-row sm:flex-wrap sm:items-start sm:justify-between sm:gap-4",
                div {
                    class: "flex min-w-0 items-start gap-3",
                    div {
                        // pb-1 keeps clearance under the mark so a sub-pixel
                        // height rounding can never crop its bottom edge.
                        class: "brand-logo w-16 shrink-0 pb-1",
                        aria_hidden: "true",
                        dangerous_inner_html: LOTUS_LOGO_SVG,
                    }
                    div {
                        class: "min-w-0 max-w-full",
                    h1 { id: PAGE_TITLE_ID,
                        class: "text-display font-bold min-w-0 break-words",
                        Link {
                             to: NavigationTarget::Internal(home.navigation_string()),

                            // `text-accent` on the chrome plane is 5.64:1 light / 7.78:1 dark,
                            // so the hover keeps 4.5:1 contrast.
                            class: "inline-flex min-h-[44px] items-center rounded-lg break-words text-text no-underline hover:text-accent focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                            "{t(locale, TextKey::PageTitle)}"
                        }
                    }
                    }
                }
                div {
                    // Wrap cleanly across narrow viewports so every route tab, language button,
                    // and theme switch remains visible, reachable, and unclipped without
                    // horizontal scrolling.
                    class: "flex max-w-full min-w-0 flex-wrap items-center gap-2 sm:justify-end sm:gap-3",
                    ViewSwitch {}
                    div {
                        class: "flex max-w-full flex-wrap items-center gap-2",
                        LangSwitch {}
                        DarkModeToggle {}
                    }
                }
            }
            p {
                // Not clamped: this sentence is the only explanation of the app a
                // first visitor gets, and the header is not sticky on phones.
                class: "mt-2.5 mb-0 break-words text-body leading-relaxed text-critical-muted sm:max-w-[72ch]",
                "{t(locale, TextKey::PageSubtitle)}"
            }
        }
    }
}
