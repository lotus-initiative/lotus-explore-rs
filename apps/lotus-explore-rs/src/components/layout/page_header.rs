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
                        // pb-1 keeps clearance under the mark so a sub-pixel
                        // height rounding can never crop its bottom edge.
                        class: "brand-logo w-16 shrink-0 pb-1",
                        aria_hidden: "true",
                        dangerous_inner_html: LOTUS_LOGO_SVG,
                    }
                    div {
                        class: "min-w-0 max-w-full",
                    h1 { id: PAGE_TITLE_ID,
                        class: "text-display font-bold min-w-0 break-words overflow-hidden",
                        Link {
                             to: NavigationTarget::Internal(home.navigation_string()),

                            // `text-accent` on the chrome plane is 5.64:1 light / 7.78:1 dark,
                            // so the hover keeps 4.5:1 contrast.
                            class: "break-words text-text no-underline hover:text-accent",
                            "{t(locale, TextKey::PageTitle)}"
                        }
                    }
                    }
                }
                div {
                    // One scrollable row on phones; squeezed groups would clip
                    // their own labels.
                    class: "flex max-w-full min-w-0 flex-nowrap items-center gap-2 overflow-x-auto pb-1 [scrollbar-width:none] [&>*]:shrink-0 sm:flex-wrap sm:justify-end sm:overflow-visible sm:pb-0",
                    ViewSwitch {}
                    LangSwitch {}
                    DarkModeToggle {}
                }
            }
            p {
                // `line-clamp-3`, not 2: German needs three lines down to 320px
                // (114 chars) and French three down to 360px (108), while English (93
                // chars) and Italian fit in two, so clamping at two truncated only two of
                // the four locales. Three is the measured maximum across all four below
                // 430px: nothing cropped, short locales still two.
                //
                // `mb-2`, not `pb-2`: the clamp's `overflow: hidden` cuts at the *padding*
                // box, so padding below a clamped element opens an 8px band inside the
                // clip and the next line's ink, starting 4px in, was sliced rather than
                // ellipsised. A margin sits outside the clip.
                class: "mt-3 line-clamp-3 break-words mb-2 text-sm leading-6 text-critical-muted sm:max-w-[72ch] sm:line-clamp-none",
                "{t(locale, TextKey::PageSubtitle)}"
            }
        }
    }
}
