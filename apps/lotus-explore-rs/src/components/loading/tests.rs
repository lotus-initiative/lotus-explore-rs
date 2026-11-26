// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `loading`, in their own file.

use super::*;
use crate::features::explore::interactions::ExploreInteractions;
use crate::features::explore::orchestrator::SearchTaskController;
use crate::repositories::hybrid::HybridRepository;
use crate::state::ResultsContext;
use crate::state::form_context::FormCriteriaContext;
use lotus_explore_core::explore::search_state::ExploreState;
use lotus_model::SearchCriteria;

#[test]
fn the_progress_line_counts_in_the_locales_own_way() {
    // The separator and the plural are the reason this reuses `format_count`
    // and `count_label` rather than formatting a number itself: 12,345 in
    // English and 12.345 in German are the same count written differently,
    // and a hand-rolled `format!` would get one of them wrong.
    assert_eq!(
        rows_so_far_text(Locale::En, 12_345),
        "12,345 Entries so far"
    );
    assert_eq!(
        rows_so_far_text(Locale::De, 12_345),
        "12.345 Einträge bisher erhalten"
    );
}

#[test]
fn a_count_below_a_thousand_is_not_grouped() {
    assert_eq!(rows_so_far_text(Locale::En, 7), "7 Entries so far");
    assert_eq!(rows_so_far_text(Locale::En, 1), "1 Entry so far");
}

/// There is deliberately no percentage, and this is why.
#[test]
fn the_progress_line_never_claims_to_know_the_total() {
    let text = rows_so_far_text(Locale::En, 12_345);
    assert!(
        !text.contains('%') && !text.contains("of"),
        "the endpoint does not say how many rows a query will produce, so any \
         denominator is invented: {text}"
    );
}

#[test]
fn preparing_phase_uses_generic_loading_title() {
    assert_eq!(
        query_phase_text(Locale::En, QueryPhase::PreparingQuery),
        query_phase_text(Locale::En, QueryPhase::Idle)
    );
}

fn render_with_contexts(component: fn() -> Element) -> String {
    let mut dom = VirtualDom::new(component);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[component]
fn LoaderSubject() -> Element {
    rsx! { LotusLoader {} }
}

#[component]
fn LoadingStateSubject() -> Element {
    use_context_provider(|| Signal::new(Locale::En));
    let explore = use_signal(ExploreState::default);
    let criteria = use_signal(|| SearchCriteria::up_to_year(2026));
    let baseline = use_signal(|| SearchCriteria::up_to_year(2026));
    let form = FormCriteriaContext::new(criteria, baseline);
    let interactions = ExploreInteractions::new(
        criteria,
        form,
        explore,
        SearchTaskController::new(),
        HybridRepository,
    );
    use_context_provider(|| ResultsContext::new(explore));
    use_context_provider(|| form);
    use_context_provider(|| interactions);
    rsx! { LoadingState {} }
}

#[component]
fn DownloadDispatchSubject() -> Element {
    use_context_provider(|| Signal::new(Locale::En));
    rsx! { DownloadDispatchState {} }
}

#[component]
fn DownloadOnlySubject() -> Element {
    use_context_provider(|| Signal::new(Locale::En));
    let explore = use_signal(ExploreState::default);
    let criteria = use_signal(|| SearchCriteria::up_to_year(2026));
    let baseline = use_signal(|| SearchCriteria::up_to_year(2026));
    let form = FormCriteriaContext::new(criteria, baseline);
    let interactions = ExploreInteractions::new(
        criteria,
        form,
        explore,
        SearchTaskController::new(),
        HybridRepository,
    );
    use_context_provider(|| ResultsContext::new(explore));
    use_context_provider(|| form);
    use_context_provider(|| interactions);
    rsx! { DownloadOnlyState {} }
}

#[test]
fn download_states_show_download_copy_rather_than_api_url_examples() {
    let dispatch_html = render_with_contexts(DownloadDispatchSubject);
    assert!(
        dispatch_html.contains(t(Locale::En, TextKey::LoadingHint)),
        "expected LoadingHint in DownloadDispatchState:\n{dispatch_html}"
    );
    assert!(!dispatch_html.contains(t(Locale::En, TextKey::ExampleApiUrls)));

    let download_only_html = render_with_contexts(DownloadOnlySubject);
    assert!(
        download_only_html.contains(t(Locale::En, TextKey::DownloadOnlyHint)),
        "expected DownloadOnlyHint in DownloadOnlyState:\n{download_only_html}"
    );
    assert!(!download_only_html.contains(t(Locale::En, TextKey::ExampleApiUrls)));
}

#[test]
fn lotus_loader_renders_flower_only_svg_with_entity_tokens() {
    let html = render_with_contexts(LoaderSubject);
    assert!(
        html.contains("<svg") && html.contains("lotus-loader"),
        "LotusLoader must render inline SVG:\n{html}"
    );
    assert!(
        !html.contains("<text"),
        "LotusLoader must omit any <text> / wordmark:\n{html}"
    );
    assert!(
        html.contains("aria-hidden=\"true\""),
        "LotusLoader SVG must be decorative (aria-hidden=true):\n{html}"
    );
    for token in [
        "var(--footer-wd-compound)",
        "var(--footer-wd-taxon)",
        "var(--footer-wd-reference)",
    ] {
        assert!(
            html.contains(token),
            "LotusLoader missing entity color token {token}:\n{html}"
        );
    }
}

#[test]
fn loading_and_download_dispatch_states_use_lotus_loader() {
    for (name, render_fn) in [
        ("LoadingState", LoadingStateSubject as fn() -> Element),
        (
            "DownloadDispatchState",
            DownloadDispatchSubject as fn() -> Element,
        ),
    ] {
        let html = render_with_contexts(render_fn);
        assert!(
            html.contains("lotus-loader"),
            "{name} must render LotusLoader:\n{html}"
        );
        assert!(
            !html.contains("spinner-lg"),
            "{name} must no longer render generic spinner-lg:\n{html}"
        );
    }
}
