// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! What the panel's reset control has to be, read from rendered HTML per the
//! `a11y_smoke` rule — a source-text assertion passes when the markup is right
//! *and* when a search-and-replace left a name in a comment. The rule is written
//! out at length in `form_sections::field_examples`.

#![allow(clippy::expect_used, clippy::panic)]

use super::*;
use crate::features::explore::ExploreInteractions;
use crate::features::explore::orchestrator::SearchTaskController;
use crate::features::explore::search_state::ExploreState;
use crate::i18n::Locale;
use crate::repositories::HybridRepository;
use crate::state::{FormCriteriaContext, ResultsContext};
use lotus_model::SearchCriteria;

/// The panel with the context it reads, and a pristine form.
#[component]
fn Subject() -> Element {
    use_context_provider(|| Signal::new(Locale::En));
    let explore = use_signal(ExploreState::default);
    let criteria = use_signal(|| SearchCriteria::up_to_year(crate::clock::current_year()));
    let baseline = use_signal(|| SearchCriteria::up_to_year(crate::clock::current_year()));
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
    rsx! {
        form { SearchPanel {} }
    }
}

/// The same, with the form already carrying a filter.
#[component]
fn FilteredSubject() -> Element {
    use_context_provider(|| Signal::new(Locale::En));
    let explore = use_signal(ExploreState::default);
    let filtered = use_signal(|| {
        let mut criteria = SearchCriteria::up_to_year(crate::clock::current_year());
        criteria.taxon = "Gentiana lutea".into();
        criteria
    });
    let baseline = use_signal(|| SearchCriteria::up_to_year(crate::clock::current_year()));
    let form = FormCriteriaContext::new(filtered, baseline);
    let interactions = ExploreInteractions::new(
        filtered,
        form,
        explore,
        SearchTaskController::new(),
        HybridRepository,
    );
    use_context_provider(|| ResultsContext::new(explore));
    use_context_provider(|| form);
    use_context_provider(|| interactions);
    rsx! {
        form { SearchPanel {} }
    }
}

fn render_of(component: fn() -> Element) -> String {
    let mut dom = VirtualDom::new(component);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

fn render() -> String {
    render_of(Subject)
}

/// The reset control, as the fragment of HTML it rendered.
fn reset_markup(html: &str) -> String {
    let fragment = html
        .split("<button")
        .find(|fragment| fragment.contains("Reset filters"))
        .unwrap_or_else(|| panic!("no reset button in the panel:\n{html}"));
    format!("<button{fragment}")
}

#[test]
fn the_panel_has_a_reset_control() {
    let html = render();
    assert!(
        html.contains("Reset filters"),
        "there is nothing to clear the filters with:\n{html}"
    );
}

#[test]
fn the_reset_does_not_submit_the_form() {
    // Load-bearing: the reset sits in the search form beside a `type="submit"`
    // button, and a button with no type in a form is a submit — so clearing the
    // filters would also run the search just cleared.
    let html = reset_markup(&render());
    assert!(
        html.contains(r#"type="button""#),
        "the reset is a plain button:\n{html}"
    );
    assert!(
        !html.contains(r#"type="submit""#),
        "and never the form's submit:\n{html}"
    );
}

#[test]
fn the_reset_is_disabled_until_something_is_set() {
    // Matched on the attribute: the class list carries `disabled:` variants
    // whatever the state.
    let pristine = reset_markup(&render());
    assert!(
        pristine.contains("disabled=true"),
        "a reset with nothing to reset is a control that lies about what the \
         page can do:\n{pristine}"
    );

    let filtered = reset_markup(&render_of(FilteredSubject));
    assert!(
        !filtered.contains("disabled=true"),
        "but a form carrying a taxon has something to clear, and a disabled \
         reset is the one case where a visitor has no way out:\n{filtered}"
    );
}

#[test]
fn the_reset_has_no_second_accessible_name() {
    // DESIGN_SYSTEM.md: visible text is the accessible name, and an `aria-label`
    // that can drift from it is a WCAG 2.5.3 failure.
    let html = reset_markup(&render());
    assert!(
        !html.contains("aria-label"),
        "the label says what it does; a second name is a second thing to keep \
         in sync:\n{html}"
    );
}

#[test]
fn the_search_button_is_still_the_submit() {
    let html = render();
    assert_eq!(
        html.matches(r#"type="submit""#).count(),
        1,
        "exactly one control submits the form:\n{html}"
    );
}
