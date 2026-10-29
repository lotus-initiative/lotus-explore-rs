// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! SimpleAnalytics visitor badge, shown as a discreet footer line beside the analytics loader.

use dioxus::prelude::*;

/// Renders the SimpleAnalytics badge for `domain`, centered in the page footer.
///
/// The badge follows the reader's color scheme: a dark variant under
/// `prefers-color-scheme: dark` and a light variant otherwise.
#[component]
pub fn SimpleAnalyticsBadge(domain: String) -> Element {
    let html = format!(
        "<a href=\"https://www.simpleanalytics.com?utm_source={domain}&utm_content=badge\" referrerpolicy=\"origin\" target=\"_blank\" rel=\"noopener noreferrer\">\
<picture>\
<source srcset=\"https://simpleanalyticsbadges.com/{domain}?mode=dark&counter=false&radius=10&background-dark=0a0f19&text-dark=b8c5d6&logo-dark=d69999\" media=\"(prefers-color-scheme: dark)\" />\
<img src=\"https://simpleanalyticsbadges.com/{domain}?mode=light&counter=false&radius=10&background-light=f7fafc&text-light=4a5a6d&logo-light=990000\" alt=\"Simple Analytics\" width=\"118\" height=\"28\" loading=\"lazy\" referrerpolicy=\"no-referrer\" crossorigin=\"anonymous\" />\
</picture>\
</a>"
    );

    rsx! {
        div { class: "flex justify-center py-2", dangerous_inner_html: "{html}" }
    }
}
