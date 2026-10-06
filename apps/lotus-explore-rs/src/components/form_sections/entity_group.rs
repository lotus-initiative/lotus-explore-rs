// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The per-entity chrome a search-filter group is wrapped in.
//!
//! A LOTUS result is always three things at once — a compound, a taxon, and a
//! reference — and every filter constrains one of them. Grouping by the thing
//! constrained rather than by the input used is what makes the set finite: three
//! groups, one obvious home per filter. The colour is the one the results table
//! and the stat bar already use, so a group and the column it filters read as the
//! same object.
//!
//! What goes inside a group is decided once, here: the primary filter is always
//! visible — the one thing almost every search needs — and everything else is a
//! *narrowing* of that, in a collapsed [`AdvancedFilters`] disclosure. Collapsed
//! by default because a dozen visible inputs was hard to scan: a mass range and a
//! publication year look equally load-bearing until read, and most searches leave
//! both alone. Nothing is removed, only relocated, so the same query is expressible
//! either way.

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
/// `advanced` is `None` for an entity with nothing to narrow, not an empty
/// disclosure: a collapsed box that opens onto nothing reads as a control that did
/// not load.
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
/// Native `<details>`, matching the SPARQL panel in the results toolbar: no toggle
/// state to keep in sync, works without JavaScript, and reports its own expanded
/// state to assistive technology. Closed on every render, so the group opens in its
/// cheapest shape and a new search does not inherit somebody else's expanded panel.
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
                // A query container, not viewport breakpoints: this panel is one
                // cell of a one-to-three column grid and can be a third of the
                // page, which a viewport-keyed layout reads as a wide screen and
                // packs eight fields into.
                class: "@container flex w-full min-w-0 flex-col gap-3 border-t border-shell-border p-2",
                {children}
            }
        }
    }
}

#[cfg(test)]
#[path = "entity_group/tests.rs"]
mod tests;
