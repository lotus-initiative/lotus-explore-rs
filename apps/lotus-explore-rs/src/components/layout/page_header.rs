// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Page header: title, language switcher, view switcher, subtitle, archive note.
//!
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
///
/// Composes `LangSwitch` (EN/FR/DE/IT), `DarkModeToggle` (light/dark), and
/// `ViewSwitch` (Search / Curation / Structure editor) as context-aware
/// children. Zero props -- only re-renders when locale or route changes.
#[component]
pub fn PageHeader() -> Element {
    let locale = use_locale();
    let route: Route = use_route();
    let home = route.with_view("landing");

    rsx! {
        header {
             // Phones get a static header: at 390px the wrapped nav plus subtitle
             // reached ~278px, which a sticky bar would keep over 40% of the
             // viewport for the whole session.
             class: "z-3 bg-shell-chrome rounded-t-xl px-4 sm:sticky sm:top-0 sm:px-8",

            div {
                // Phones: brand on its own row, nav on a scrollable row below.
                // sm and up: the original single wrapped row.
                class: "flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-start sm:justify-between sm:gap-4",
                div {
                    class: "flex min-w-0 items-start gap-3",
                    div {
                        class: "w-16 shrink-0",
                        aria_hidden: "true",
                        dangerous_inner_html: LOTUS_LOGO_SVG,
                    }
                    div {
                        class: "min-w-0 max-w-full",
                    h1 { id: PAGE_TITLE_ID,
                        class: "text-display font-bold min-w-0 break-words overflow-hidden",
                        Link {
                             to: NavigationTarget::Internal(home.navigation_string()),

                            class: "break-words text-text no-underline hover:no-underline",
                            "{t(locale, TextKey::PageTitle)}"
                        }
                    }
                    }
                }
                div {
                    // One scrollable row on phones instead of three wrapped ones.
                    // The groups keep their natural width: squeezed, each would
                    // clip its own labels.
                    class: "flex max-w-full min-w-0 flex-nowrap items-center gap-2 overflow-x-auto pb-1 [scrollbar-width:none] [&>*]:shrink-0 sm:flex-wrap sm:justify-end sm:overflow-visible sm:pb-0",
                    ViewSwitch {}
                    LangSwitch {}
                    DarkModeToggle {}
                }
            }
            p {
                class: "mt-3 line-clamp-2 break-words pb-2 text-sm leading-6 text-critical-muted sm:max-w-[72ch] sm:line-clamp-none",
                "{t(locale, TextKey::PageSubtitle)}"
            }
        }
    }
}
