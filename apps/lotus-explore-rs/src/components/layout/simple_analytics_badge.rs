// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! `SimpleAnalytics` visitor badge, shown as a discreet footer line alongside the no-JS analytics pixel.

use crate::state::AppStateContext;
use dioxus::prelude::*;

/// Renders the `SimpleAnalytics` badge for `domain`, centered in the page footer.
///
/// The badge follows the active `AppStateContext` theme (URL / saved preference /
/// toggle / OS fallback) and matches the shell chrome plane background.
#[component]
pub fn SimpleAnalyticsBadge(domain: String) -> Element {
    let dark_mode =
        try_consume_context::<AppStateContext>().is_some_and(|ctx| ctx.state.read().dark_mode);
    let badge_src = if dark_mode {
        format!(
            "https://simpleanalyticsbadges.com/{domain}?mode=dark&counter=false&radius=10&background-dark=1c2637&text-dark=b8c5d6&logo-dark=d69999"
        )
    } else {
        format!(
            "https://simpleanalyticsbadges.com/{domain}?mode=light&counter=false&radius=10&background-light=e7ecf1&text-light=4a5a6d&logo-light=990000"
        )
    };
    let href = format!("https://www.simpleanalytics.com?utm_source={domain}&utm_content=badge");

    rsx! {
        div { class: "flex justify-center py-1",
            a {
                class: "inline-flex min-h-[44px] min-w-[44px] items-center justify-center rounded-xl px-2 focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                href: "{href}",
                "referrerpolicy": "origin",
                target: "_blank",
                rel: "noopener noreferrer",
                img {
                    src: "{badge_src}",
                    alt: "Simple Analytics",
                    width: "118",
                    height: "28",
                    loading: "lazy",
                    "referrerpolicy": "no-referrer",
                    crossorigin: "anonymous",
                }
            }
        }
    }
}
