// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Small presentation helpers shared across the network graph workspace components.

use crate::i18n::{TextKey, t};
use dioxus::prelude::*;
use lotus_explore_core::graph::{GraphProjection, NodeKind};

/// Localized display label for an entity [`NodeKind`].
pub(super) fn entity_kind_label(locale: crate::i18n::Locale, kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Compound => t(locale, TextKey::Compound),
        NodeKind::Taxon => t(locale, TextKey::Taxon),
        NodeKind::Reference => t(locale, TextKey::Reference),
    }
}

/// Semantic Wikidata badge classes (tested for >=4.8:1 contrast in light & dark).
pub(super) const fn entity_badge_classes(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Compound => "border-stat-compound-border bg-stat-compound text-text-wd-compound",
        NodeKind::Taxon => "border-stat-taxon-border bg-stat-taxon text-text-wd-taxon",
        NodeKind::Reference => {
            "border-stat-reference-border bg-stat-reference text-text-wd-reference"
        }
    }
}

/// Entity fill CSS custom property (theme-aware light & dark).
pub(super) const fn entity_css_color(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Compound => "var(--footer-wd-compound)",
        NodeKind::Taxon => "var(--footer-wd-taxon)",
        NodeKind::Reference => "var(--footer-wd-reference)",
    }
}

/// Localized label for a [`GraphProjection`].
pub(super) fn projection_label(
    locale: crate::i18n::Locale,
    projection: GraphProjection,
) -> &'static str {
    match projection {
        GraphProjection::Auto => t(locale, TextKey::ProjectionAuto),
        GraphProjection::CompoundTaxon => t(locale, TextKey::ProjectionCompoundTaxon),
        GraphProjection::CompoundReference => t(locale, TextKey::ProjectionCompoundReference),
        GraphProjection::TaxonReference => t(locale, TextKey::ProjectionTaxonReference),
        GraphProjection::Tripartite => t(locale, TextKey::ProjectionTripartite),
    }
}

/// Wikidata, Scholia, and optional DOI target URLs for a node.
pub(super) fn wikidata_url(qid: &str) -> String {
    format!("https://www.wikidata.org/entity/{qid}")
}

/// Scholia profile URL corresponding to `kind`.
pub(super) fn scholia_url(kind: NodeKind, qid: &str) -> String {
    let segment = match kind {
        NodeKind::Compound => "chemical",
        NodeKind::Taxon => "taxon",
        NodeKind::Reference => "work",
    };
    format!("https://scholia.toolforge.org/{segment}/{qid}")
}

/// Localized aria key describing the Scholia target for `kind`.
pub(super) const fn scholia_aria_key(kind: NodeKind) -> TextKey {
    match kind {
        NodeKind::Compound => TextKey::OpenInCompoundScholia,
        NodeKind::Taxon => TextKey::OpenInTaxonScholia,
        NodeKind::Reference => TextKey::OpenInReferenceScholia,
    }
}

/// Corroboration unit label (`refs`, `taxa`, or `compounds`) for an edge in `projection`.
pub(super) fn corroboration_unit(
    locale: crate::i18n::Locale,
    projection: GraphProjection,
) -> &'static str {
    match projection {
        GraphProjection::Auto | GraphProjection::CompoundTaxon => {
            t(locale, TextKey::GraphCorroborationRefs)
        }
        GraphProjection::CompoundReference => t(locale, TextKey::GraphCorroborationTaxa),
        GraphProjection::TaxonReference => t(locale, TextKey::GraphCorroborationCompounds),
        GraphProjection::Tripartite => t(locale, TextKey::NodeOccurrencesLabel),
    }
}

/// Summarize active search criteria concisely for the graph toolbar chip.
pub(super) fn criteria_summary(criteria: &lotus_model::SearchCriteria) -> String {
    let mut parts: Vec<String> = Vec::new();
    let taxon = criteria.taxon.trim();
    if !taxon.is_empty() && taxon != "*" {
        parts.push(taxon.to_owned());
    }
    let structure = criteria.structure.trim();
    if !structure.is_empty() {
        parts.push(structure.to_owned());
    }
    let reference = criteria.reference.trim();
    if !reference.is_empty() {
        parts.push(reference.to_owned());
    }
    let formula = criteria.formula_exact.trim();
    if criteria.formula_enabled && !formula.is_empty() {
        parts.push(formula.to_owned());
    }
    if parts.is_empty() {
        String::from("*")
    } else {
        parts.join(" · ")
    }
}

/// Accessible SVG glyph matching the WebGL2 SDF vertex shape:
/// Compound = circle, Taxon = diamond, Reference = rounded square.
#[component]
pub(super) fn EntityShapeIcon(kind: NodeKind) -> Element {
    let color = entity_css_color(kind);
    rsx! {
        svg {
            view_box: "0 0 16 16",
            class: "size-3.5 shrink-0",
            "aria-hidden": "true",
            "focusable": "false",
            match kind {
                NodeKind::Compound => rsx! {
                    circle { cx: "8", cy: "8", r: "5.5", fill: "{color}" }
                },
                NodeKind::Taxon => rsx! {
                    polygon { points: "8,1.5 14.5,8 8,14.5 1.5,8", fill: "{color}" }
                },
                NodeKind::Reference => rsx! {
                    rect { x: "2.5", y: "2.5", width: "11", height: "11", rx: "2.5", fill: "{color}" }
                },
            }
        }
    }
}
