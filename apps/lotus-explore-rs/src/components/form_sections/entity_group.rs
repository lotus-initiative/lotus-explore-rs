// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The per-entity chrome a search-filter group is wrapped in.
//!
//! A LOTUS result is always three things at once — a compound, a taxon, and a
//! reference — and every filter constrains one of them. Grouping the controls by
//! the thing they constrain, rather than by the input they happen to use, is
//! what makes the set finite: three groups, and each filter has exactly one
//! obvious home. The colour is the one the results table and the stat bar
//! already use for each of the three, so a group and the column it filters are
//! recognisably the same object.
//!
//! What belongs inside a group is decided once, here:
//!
//! - the group's own primary filter is always visible — the one thing almost
//!   every search needs;
//! - everything else is a *narrowing* of that, and goes into a collapsed
//!   [`AdvancedFilters`] disclosure. Collapsed by default, because a dozen
//!   visible inputs is what made the previous layout hard to scan: a mass range
//!   and a publication year look equally load-bearing until you read them, and
//!   most searches leave both alone.
//!
//! Nothing is removed, only relocated, so the same query is expressible either
//! way.

use crate::i18n::{TextKey, t};
use dioxus::prelude::*;

/// The three things a result row is about, and therefore the three filter
/// groups there are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterEntity {
    Compound,
    Taxon,
    Reference,
}

impl FilterEntity {
    /// The group's heading. Already a `TextKey`, because the three names are
    /// the same words the table's column headers use.
    pub const fn label(self) -> TextKey {
        match self {
            Self::Compound => TextKey::Compound,
            Self::Taxon => TextKey::Taxon,
            Self::Reference => TextKey::Reference,
        }
    }

    /// The entity colour token, without the `text-` prefix, so one `match`
    /// serves both the heading and the stripe instead of two that can drift.
    pub const fn color(self) -> &'static str {
        match self {
            Self::Compound => "wd-compound",
            Self::Taxon => "wd-taxon",
            Self::Reference => "wd-reference",
        }
    }

    /// The `id` stem for the group's own heading and for its disclosure, so the
    /// `aria-labelledby` and the `id` cannot name different things.
    pub const fn id_stem(self) -> &'static str {
        match self {
            Self::Compound => "compound-filters",
            Self::Taxon => "taxon-filters",
            Self::Reference => "reference-filters",
        }
    }

    fn heading_id(self) -> String {
        format!("{}-heading", self.id_stem())
    }

    fn accent_class(self) -> String {
        format!("text-{}", self.color())
    }

    fn stripe_class(self) -> String {
        format!("border-l-4 border-l-{}", self.color())
    }
}

/// One entity's filter group: a named, coloured box holding the primary filter
/// and, when there is one, a collapsed set of advanced sub-filters.
///
/// `advanced` is `None` for an entity that has nothing to narrow, rather than an
/// empty disclosure: a collapsed box that opens onto nothing reads as a control
/// that did not load.
#[component]
pub fn EntityFilters(entity: FilterEntity, primary: Element, advanced: Option<Element>) -> Element {
    let locale = crate::hooks::use_locale();
    let heading_id = entity.heading_id();

    rsx! {
        div {
            class: "flex min-w-0 flex-col gap-2 rounded-xl border border-shell-border {entity.stripe_class()} bg-shell-raised p-2.5",
            role: "group",
            aria_labelledby: "{heading_id}",
            p {
                id: "{heading_id}",
                class: "text-body font-semibold {entity.accent_class()}",
                "{t(locale, entity.label())}"
            }
            {primary}
            if let Some(advanced) = advanced {
                AdvancedFilters {
                    entity,
                    summary_id: format!("{}-advanced-summary", entity.id_stem()),
                    body_id: format!("{}-advanced-body", entity.id_stem()),
                    {advanced}
                }
            }
        }
    }
}

/// The collapsed disclosure holding one group's advanced sub-filters.
///
/// Native `<details>`, matching the SPARQL panel in the results toolbar: no
/// toggle state to keep in sync, works without JavaScript, and reports its own
/// expanded state to assistive technology. It is closed on every render, so the
/// group opens in its cheapest shape and a new search does not inherit somebody
/// else's expanded panel.
#[component]
fn AdvancedFilters(
    entity: FilterEntity,
    summary_id: String,
    body_id: String,
    children: Element,
) -> Element {
    let locale = crate::hooks::use_locale();

    rsx! {
        details {
            class: "group overflow-hidden rounded-xl border border-shell-border bg-panel-soft",
            summary {
                id: "{summary_id}",
                class: "flex w-auto min-w-0 cursor-pointer select-none items-center gap-1.5 px-2 py-1.5 text-ui font-semibold text-muted hover:bg-bg focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
                span { class: "{entity.accent_class()}", "{t(locale, TextKey::AdvancedFilters)}" }
                span {
                    class: "inline-block text-subtle transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] group-open:rotate-90",
                    aria_hidden: "true",
                    "▶"
                }
            }
            div {
                id: "{body_id}",
                // A query container, because what is inside this panel is sized
                // by the panel and not by the window. The group is one cell of a
                // one-to-three column grid, so at a desktop width it can be a
                // third of the page -- and a layout keyed to viewport breakpoints
                // reads that narrow column as a wide screen and packs eight
                // fields into it.
                class: "@container flex w-full min-w-0 flex-col gap-3 border-t border-shell-border p-2",
                {children}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::i18n::Locale;

    /// Every group, in the order the results table's columns read left to right.
    const ALL: [FilterEntity; 3] = [
        FilterEntity::Compound,
        FilterEntity::Taxon,
        FilterEntity::Reference,
    ];

    /// The three groups and their colours have to be the ones the rest of the app
    /// already uses; a group painted in a fourth colour is a group nobody can
    /// match to a column.
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
    // Asserting on rendered HTML rather than on the source, for the reason
    // `a11y_smoke` records: a class name that was meant to be applied and was
    // not is invisible to a source-text check and obvious to this one.

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
        // Collapsed by default only works if the content is still in the
        // document; hiding it with CSS would leave it focusable and announced.
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
}
