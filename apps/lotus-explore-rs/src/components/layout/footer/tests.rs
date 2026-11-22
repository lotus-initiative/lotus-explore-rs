// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit tests for the footer link wrapping and analytics badge theme synchronization.

use super::Footer;
use crate::app_state::AppState;
use crate::components::layout::simple_analytics_badge::SimpleAnalyticsBadge;
use crate::i18n::Locale;
use crate::state::AppStateContext;
use dioxus::prelude::*;

#[component]
fn FooterSubject() -> Element {
    use_context_provider(|| Signal::new(Locale::En));
    rsx! { Footer {} }
}

#[derive(Clone, PartialEq)]
struct BadgeProps {
    dark_mode: bool,
}

#[component]
fn BadgeSubject(props: BadgeProps) -> Element {
    let state = use_signal(|| AppState {
        dark_mode: props.dark_mode,
        ..AppState::default()
    });
    use_context_provider(|| AppStateContext::new(state));
    rsx! { SimpleAnalyticsBadge { domain: "example.org".to_string() } }
}

fn render_footer() -> String {
    let mut dom = VirtualDom::new(FooterSubject);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

fn render_badge(dark_mode: bool) -> String {
    let mut dom = VirtualDom::new_with_props(BadgeSubject, BadgeProps { dark_mode });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn footer_links_wrap_without_horizontal_scroll_clipping() {
    let html = render_footer();
    assert!(
        html.contains("flex-wrap"),
        "expected wrapping footer lists:\n{html}"
    );
    assert!(
        !html.contains("scrollbar-hidden-x"),
        "footer must not hide overflow behind an unlabelled horizontal scroller:\n{html}"
    );
    assert!(
        !html.contains("overflow-x-auto"),
        "footer must wrap instead of scrolling horizontally:\n{html}"
    );
    assert!(
        !html.contains("flex-nowrap"),
        "footer groups must allow wrapping on narrow viewports:\n{html}"
    );
    assert!(
        html.contains("min-h-[44px]"),
        "footer links must meet 44px touch target height:\n{html}"
    );
    for expected in ["LOTUS Frozen", "QLever SPARQL", "Sachem", "AGPL-3.0"] {
        assert!(html.contains(expected), "missing footer link {expected}");
    }
}

#[test]
fn analytics_badge_follows_app_dark_mode_state_and_chrome_plane() {
    let light_html = render_badge(false);
    assert!(
        light_html.contains("mode=light") && light_html.contains("background-light=e7ecf1"),
        "expected light chrome badge:\n{light_html}"
    );
    assert!(
        light_html.contains("min-h-[44px]"),
        "badge link should meet 44px touch target:\n{light_html}"
    );

    let dark_html = render_badge(true);
    assert!(
        dark_html.contains("mode=dark") && dark_html.contains("background-dark=1c2637"),
        "expected dark chrome badge matching --shell-chrome-bg:\n{dark_html}"
    );
}
