// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `entity_group`, in their own file.

#![allow(clippy::expect_used)]

use super::*;
use crate::i18n::Locale;

/// Every group, in the order the results table's columns read left to right.
const ALL: [FilterEntity; 3] = [
    FilterEntity::Compound,
    FilterEntity::Taxon,
    FilterEntity::Reference,
];

/// The three groups and colours are the ones the rest of the app already
/// uses; a fourth colour is a group nobody can match to a column.
#[test]
fn each_entity_maps_to_its_existing_wikidata_colour() {
    assert_eq!(FilterEntity::Compound.color(), "wd-compound");
    assert_eq!(FilterEntity::Taxon.color(), "wd-taxon");
    assert_eq!(FilterEntity::Reference.color(), "wd-reference");
}

#[test]
fn heading_ids_are_unique_per_group() {
    let ids = ALL.map(FilterEntity::heading_id);
    assert_eq!(
        ids,
        [
            "compound-filters-heading",
            "taxon-filters-heading",
            "reference-filters-heading"
        ]
    );
}

#[test]
fn accent_and_stripe_derive_from_the_same_token() {
    for entity in ALL {
        let color = entity.color();
        assert!(entity.accent_class().ends_with(color));
        assert!(entity.stripe_class().ends_with(color));
    }
}

// ── What the markup actually says ────────────────────────────────────────
//
// Rendered HTML, not source, per `a11y_smoke`: a class name meant to be
// applied and not applied is invisible to a source-text check.

#[component]
fn WithAdvanced() -> Element {
    use_context_provider(|| Signal::new(Locale::En));
    let primary = rsx! { input { id: "primary-input" } };
    let advanced = rsx! { input { id: "advanced-input" } };
    rsx! {
        EntityFilters {
            entity: FilterEntity::Taxon,
            primary,
            advanced: Some(advanced),
        }
    }
}

#[component]
fn WithoutAdvanced() -> Element {
    use_context_provider(|| Signal::new(Locale::En));
    let primary = rsx! { input { id: "primary-input" } };
    rsx! {
        EntityFilters {
            entity: FilterEntity::Reference,
            primary,
            advanced: None,
        }
    }
}

fn render_with_advanced() -> String {
    let mut dom = VirtualDom::new(WithAdvanced);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

fn render_without_advanced() -> String {
    let mut dom = VirtualDom::new(WithoutAdvanced);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn the_group_is_a_named_group_and_the_heading_it_names_is_on_the_page() {
    let html = render_with_advanced();
    assert!(
        html.contains(r#"role="group""#),
        "the group is announced as a group:\n{html}"
    );
    assert!(
        html.contains(r#"aria-labelledby="taxon-filters-heading""#),
        "and named by its heading, not by nothing:\n{html}"
    );
    assert!(
        html.contains(r#"id="taxon-filters-heading""#),
        "the heading carries that id:\n{html}"
    );
}

#[test]
fn the_group_is_painted_in_the_entity_colour() {
    let html = render_with_advanced();
    assert!(
        html.contains("border-l-wd-taxon"),
        "the taxon group is striped in the taxon colour:\n{html}"
    );
    assert!(
        html.contains("text-wd-taxon"),
        "and its heading is in that colour too:\n{html}"
    );
}

#[test]
fn the_advanced_filters_start_collapsed() {
    let html = render_with_advanced();
    assert!(
        html.contains("<details"),
        "advanced filters are a native disclosure:\n{html}"
    );
    assert!(
        !html.contains("<details open"),
        "and it starts closed, so a group opens in its cheapest shape:\n{html}"
    );
    // Collapsed by default only works with the content still in the document:
    // CSS hiding would leave it focusable and announced.
    assert!(
        html.contains(r#"id="advanced-input""#),
        "the advanced controls are rendered, just not shown:\n{html}"
    );
}

#[test]
fn a_group_with_nothing_to_narrow_has_no_disclosure_at_all() {
    let html = render_without_advanced();
    assert!(
        !html.contains("<details"),
        "a collapsed panel that opens onto nothing reads as a control that \
         did not load:\n{html}"
    );
    assert!(
        html.contains(r#"id="primary-input""#),
        "but the primary filter is still there:\n{html}"
    );
}
