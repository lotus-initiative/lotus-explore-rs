// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Entity inspector & Top Connected Hubs side panel for `/network`.
//!
//! Reads structure SMILES, formula, mass, `InChIKey`, and DOIs straight from
//! [`lotus_model::ColumnarResultSet`] with zero extra network calls, and reuses
//! [`crate::components::results_table::depict_url_for_smiles`] for stereo-aware
//! 2D CDK Depict SVGs.

use super::helpers::{
    EntityShapeIcon, corroboration_unit, entity_badge_classes, entity_kind_label, projection_label,
    scholia_aria_key, scholia_url, wikidata_url,
};
use crate::components::copy_button::CopyButton;
use crate::components::results_table::depict_url_for_smiles;
use crate::components::ui::Button;
use crate::i18n::{Locale, TextKey, format_count, t};
use dioxus::prelude::*;
use lotus_explore_core::filters::ColumnFilters;
use lotus_explore_core::graph::{GraphModel, GraphNode, GraphScope, NodeKey, NodeKind};
use std::sync::Arc;

const MAX_RENDERED_NEIGHBORS: usize = 200;

/// Build table [`ColumnFilters`] isolating `node` when returning to `/search`.
pub(super) fn table_filter_for_node(
    node: &GraphNode,
    set: &lotus_model::ColumnarResultSet,
) -> ColumnFilters {
    let mut filters = ColumnFilters::empty();
    match node.key.kind {
        NodeKind::Compound => {
            set.compound_label(node.sample_row)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(&node.qid_text)
                .clone_into(&mut filters.compound);
        }
        NodeKind::Taxon => {
            set.taxon_label(node.sample_row)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(&node.qid_text)
                .clone_into(&mut filters.taxon);
        }
        NodeKind::Reference => {
            set.reference_title(node.sample_row)
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(&node.qid_text)
                .clone_into(&mut filters.reference);
        }
    }
    filters
}

/// Entity inspector (when a node is selected) or Top Connected Entities list.
#[component]
pub(super) fn GraphInspector(
    model: Arc<GraphModel>,
    set_arc: Arc<lotus_model::ColumnarResultSet>,
    scope: GraphScope,
    selected_idx: Option<usize>,
    on_select: EventHandler<Option<usize>>,
    on_focus_ego: EventHandler<NodeKey>,
    on_back_to_overview: EventHandler<()>,
    on_filter_table: EventHandler<ColumnFilters>,
    on_pivot: EventHandler<(NodeKind, String)>,
) -> Element {
    let locale = crate::hooks::use_locale();
    let selected_node = selected_idx.and_then(|idx| model.nodes.get(idx).map(|n| (idx, n)));

    let primary_note = if let Some((_, node)) = selected_node {
        format!(
            "{}: {} ({})",
            entity_kind_label(locale, node.key.kind),
            node.label,
            node.qid_text
        )
    } else {
        format!(
            "{}: {}",
            t(locale, TextKey::TopConnectedEntities),
            format_count(locale, model.top_hubs.len())
        )
    };
    let mut announcement_parts = vec![
        primary_note,
        String::from(projection_label(locale, model.effective_projection)),
    ];
    if matches!(scope, GraphScope::Ego { .. })
        && let Some(center) = model.nodes.first()
    {
        announcement_parts.push(format!(
            "{} {}",
            t(locale, TextKey::GraphEgoBadgePrefix),
            center.label
        ));
    }
    if model.capped {
        announcement_parts.push(format!(
            "{} {} / {}",
            t(locale, TextKey::GraphCappedNoticePrefix),
            format_count(locale, model.nodes.len()),
            format_count(locale, model.total_distinct_nodes)
        ));
    }
    let live_announcement = announcement_parts.join(" · ");

    rsx! {
        aside {
            role: "region",
            aria_label: "{t(locale, TextKey::NetworkInspectorRegionAria)}",
            class: "flex min-w-0 flex-col gap-3 rounded-xl border border-panel-border bg-panel p-4 shadow-xs lotus-graph-inspector",
            p {
                role: "status",
                aria_live: "polite",
                class: "sr-only",
                "{live_announcement}"
            }

            if let Some((idx, node)) = selected_node {
                SelectedNodeView {
                    model: model.clone(),
                    set_arc,
                    scope,
                    node_idx: idx,
                    node: node.clone(),
                    locale,
                    on_clear: move |()| on_select.call(None),
                    on_select_neighbor: move |n_idx| on_select.call(Some(n_idx)),
                    on_focus_ego,
                    on_back_to_overview,
                    on_filter_table,
                    on_pivot,
                }
            } else {
                TopHubsList {
                    model: model.clone(),
                    locale,
                    on_pick: move |idx| on_select.call(Some(idx)),
                }
            }
        }
    }
}

#[component]
fn TopHubsList(model: Arc<GraphModel>, locale: Locale, on_pick: EventHandler<usize>) -> Element {
    rsx! {
        div { class: "flex flex-col gap-2.5",
            h3 { class: "m-0 text-body font-semibold text-text",
                "{t(locale, TextKey::TopConnectedEntities)}"
            }
            if model.top_hubs.is_empty() {
                p { class: "m-0 text-body text-muted", "{t(locale, TextKey::NoResults)}" }
            } else {
                ul { class: "m-0 flex list-none flex-col gap-1.5 p-0",
                    for &hub_idx in &model.top_hubs {
                        if let Some(hub) = model.nodes.get(hub_idx) {
                            HubButtonRow {
                                hub_idx,
                                node: hub.clone(),
                                locale,
                                on_pick,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn HubButtonRow(
    hub_idx: usize,
    node: GraphNode,
    locale: Locale,
    on_pick: EventHandler<usize>,
) -> Element {
    let badge = entity_badge_classes(node.key.kind);
    let label_cls = if node.key.kind == NodeKind::Taxon {
        "min-w-0 flex-1 truncate italic text-ui font-medium text-text"
    } else {
        "min-w-0 flex-1 truncate text-ui font-medium text-text"
    };
    let deg_str = format_count(locale, usize::try_from(node.degree).unwrap_or(0));

    rsx! {
        li {
            button {
                r#type: "button",
                class: "flex min-h-[44px] w-full cursor-pointer items-center gap-2 rounded-xl border border-panel-border bg-surface px-3 py-2 text-left transition-colors hover:border-border hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                onclick: move |_| on_pick.call(hub_idx),
                EntityShapeIcon { kind: node.key.kind }
                span { class: label_cls, "{node.label}" }
                span { class: "shrink-0 rounded-full border px-2 py-0.5 text-micro font-semibold tabular-nums {badge}",
                    "{deg_str}"
                }
            }
        }
    }
}

#[component]
fn SelectedNodeView(
    model: Arc<GraphModel>,
    set_arc: Arc<lotus_model::ColumnarResultSet>,
    scope: GraphScope,
    node_idx: usize,
    node: GraphNode,
    locale: Locale,
    on_clear: EventHandler<()>,
    on_select_neighbor: EventHandler<usize>,
    on_focus_ego: EventHandler<NodeKey>,
    on_back_to_overview: EventHandler<()>,
    on_filter_table: EventHandler<ColumnFilters>,
    on_pivot: EventHandler<(NodeKind, String)>,
) -> Element {
    let badge = entity_badge_classes(node.key.kind);
    let kind_label = entity_kind_label(locale, node.key.kind);
    let is_ego_center = matches!(scope, GraphScope::Ego { center, .. } if center == node.key);

    let title_class = if node.key.kind == NodeKind::Taxon {
        "m-0 text-body font-semibold italic text-text break-words"
    } else {
        "m-0 text-body font-semibold text-text break-words"
    };

    let qid = node.qid_text.clone();
    let wd_href = wikidata_url(&qid);
    let scholia_href = scholia_url(node.key.kind, &qid);
    let scholia_aria = t(locale, scholia_aria_key(node.key.kind));

    let smiles = (node.key.kind == NodeKind::Compound)
        .then(|| set_arc.smiles(node.sample_row))
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned);
    let inchikey = (node.key.kind == NodeKind::Compound)
        .then(|| set_arc.inchikey(node.sample_row))
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned);
    let depict_url = smiles.as_deref().and_then(depict_url_for_smiles);

    let doi = (node.key.kind == NodeKind::Reference)
        .then(|| set_arc.reference_doi(node.sample_row))
        .flatten()
        .filter(|d| !d.trim().is_empty())
        .map(str::to_owned);

    let mut neighbors = model.neighbors_of(node_idx);
    let total_neighbors = neighbors.len();
    neighbors.truncate(MAX_RENDERED_NEIGHBORS);
    let unit = corroboration_unit(locale, model.effective_projection);

    let deg_str = format_count(locale, usize::try_from(node.degree).unwrap_or(0));
    let occ_str = format_count(locale, usize::try_from(node.occurrence_count).unwrap_or(0));
    let qid_for_pivot = qid.clone();
    let node_for_filter = node.clone();
    let set_for_filter = set_arc;

    rsx! {
        div { class: "flex flex-col gap-3",
            div { class: "flex items-center justify-between gap-2",
                span { class: "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-micro font-semibold {badge}",
                    EntityShapeIcon { kind: node.key.kind }
                    "{kind_label}"
                }
                Button {
                    r#type: "button",
                    label: t(locale, TextKey::CloseInspector).to_string(),
                    class: "inline-flex min-h-[44px] min-w-[44px] items-center justify-center rounded-lg border border-border bg-surface px-3 py-2 text-ui font-semibold text-text hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                    onclick: move |_| on_clear.call(()),
                }
            }

            div { class: "flex flex-col gap-1",
                h3 { class: title_class, "{node.label}" }
                div { class: "flex flex-wrap items-center gap-2 text-micro text-subtle",
                    span { class: "font-mono", "{qid}" }
                    if let Some(sec) = node.secondary.as_deref() {
                        span { "· {sec}" }
                    }
                }
            }

            div { class: "grid grid-cols-2 gap-2 rounded-xl border border-panel-border bg-panel-soft p-2.5 text-ui",
                div { class: "flex flex-col",
                    span { class: "text-micro text-subtle", "{t(locale, TextKey::NodeDegreeLabel)}" }
                    span { class: "font-semibold text-text tabular-nums", "{deg_str}" }
                }
                div { class: "flex flex-col",
                    span { class: "text-micro text-subtle", "{t(locale, TextKey::NodeOccurrencesLabel)}" }
                    span { class: "font-semibold text-text tabular-nums", "{occ_str}" }
                }
            }

            if let Some(url) = depict_url {
                div { class: "flex flex-col gap-2 rounded-xl border border-panel-border bg-panel-soft p-2.5",
                    img {
                        src: "{url}",
                        alt: "{node.label}",
                        class: "mx-auto max-h-40 w-auto rounded-lg bg-white p-1.5",
                    }
                    if let Some(ref sm) = smiles {
                        div { class: "flex items-center justify-between gap-2",
                            span { class: "min-w-0 flex-1 truncate font-mono text-micro text-muted",
                                title: "{sm}",
                                "{t(locale, TextKey::GraphSmilesLabel)}: {sm}"
                            }
                            CopyButton {
                                text: Arc::<str>::from(sm.as_str()),
                                label: t(locale, TextKey::CopySmiles),
                                title: t(locale, TextKey::CopySmiles),
                                locale,
                            }
                        }
                    }
                    if let Some(ref ik) = inchikey {
                        span { class: "font-mono text-micro text-subtle break-all",
                            "{ik}"
                        }
                    }
                }
            } else if let Some(ref sm) = smiles {
                div { class: "flex items-center justify-between gap-2 rounded-xl border border-panel-border bg-panel-soft p-2.5",
                    span { class: "min-w-0 flex-1 truncate font-mono text-micro text-muted",
                        "{t(locale, TextKey::GraphSmilesLabel)}: {sm}"
                    }
                    CopyButton {
                        text: Arc::<str>::from(sm.as_str()),
                        label: t(locale, TextKey::CopySmiles),
                        title: t(locale, TextKey::CopySmiles),
                        locale,
                    }
                }
            }

            div { class: "flex flex-wrap items-center gap-2",
                a {
                    href: "{wd_href}",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    aria_label: "{qid} · Wikidata — {t(locale, TextKey::OpenInWikidata)}",
                    class: "inline-flex min-h-[44px] min-w-[44px] items-center rounded-full border border-border bg-surface px-3 py-2 text-micro font-semibold text-text hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                    "{qid} · Wikidata"
                }
                a {
                    href: "{scholia_href}",
                    target: "_blank",
                    rel: "noopener noreferrer",
                    aria_label: "{qid} · Scholia — {scholia_aria}",
                    class: "inline-flex min-h-[44px] min-w-[44px] items-center rounded-full border border-border bg-surface px-3 py-2 text-micro font-semibold text-text hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                    "{qid} · Scholia"
                }
                if let Some(ref d) = doi {
                    a {
                        href: "https://doi.org/{d}",
                        target: "_blank",
                        rel: "noopener noreferrer",
                        aria_label: "{d} — {t(locale, TextKey::OpenDoi)}",
                        class: "inline-flex min-h-[44px] min-w-[44px] items-center rounded-full border border-stat-reference-border bg-surface px-3 py-2 text-micro font-semibold text-text-wd-reference hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                        "{d}"
                    }
                }
            }

            div { class: "flex flex-col gap-2 pt-1",
                Button {
                    r#type: "button",
                    label: t(locale, TextKey::ExploreInLotus).to_string(),
                    class: "inline-flex w-full items-center justify-center rounded-xl bg-primary px-3 py-2 text-ui font-semibold text-white shadow-xs hover:bg-primary-hover focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 min-h-[44px]",
                    onclick: move |_| on_pivot.call((node.key.kind, qid_for_pivot.clone())),
                }
                div { class: "grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-1",
                    if is_ego_center {
                        Button {
                            r#type: "button",
                            label: t(locale, TextKey::GraphBackToOverview).to_string(),
                            class: "inline-flex items-center justify-center rounded-xl border border-border bg-surface px-3 py-2 text-ui font-semibold text-text shadow-xs hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 min-h-[44px]",
                            onclick: move |_| on_back_to_overview.call(()),
                        }
                    } else {
                        Button {
                            r#type: "button",
                            label: t(locale, TextKey::FocusNeighborhood).to_string(),
                            class: "inline-flex items-center justify-center rounded-xl border border-border bg-surface px-3 py-2 text-ui font-semibold text-text shadow-xs hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 min-h-[44px]",
                            onclick: move |_| on_focus_ego.call(node.key),
                        }
                    }
                    Button {
                        r#type: "button",
                        label: t(locale, TextKey::FilterTableToEntity).to_string(),
                        class: "inline-flex items-center justify-center rounded-xl border border-border bg-surface px-3 py-2 text-ui font-semibold text-text shadow-xs hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 min-h-[44px]",
                        onclick: move |_| {
                            let f = table_filter_for_node(&node_for_filter, &set_for_filter);
                            on_filter_table.call(f);
                        },
                    }
                }
            }

            div { class: "flex flex-col gap-1.5 border-t border-panel-border pt-3",
                h4 { class: "m-0 text-ui font-semibold text-text",
                    "{t(locale, TextKey::LinkedEntities)} ({format_count(locale, total_neighbors)})"
                }
                if neighbors.is_empty() {
                    p { class: "m-0 text-body text-subtle", "{t(locale, TextKey::NoResults)}" }
                } else {
                    ul { class: "m-0 flex list-none flex-col gap-1 p-0",
                        for (n_idx, weight) in neighbors {
                            if let Some(nbr) = model.nodes.get(n_idx) {
                                NeighborRowButton {
                                    neighbor_idx: n_idx,
                                    neighbor: nbr.clone(),
                                    weight,
                                    unit,
                                    locale,
                                    on_pick: on_select_neighbor,
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn NeighborRowButton(
    neighbor_idx: usize,
    neighbor: GraphNode,
    weight: u32,
    unit: &'static str,
    locale: Locale,
    on_pick: EventHandler<usize>,
) -> Element {
    let label_cls = if neighbor.key.kind == NodeKind::Taxon {
        "min-w-0 flex-1 truncate italic text-text"
    } else {
        "min-w-0 flex-1 truncate text-text"
    };
    let w_str = format_count(locale, usize::try_from(weight).unwrap_or(0));

    rsx! {
        li {
            button {
                r#type: "button",
                class: "flex min-h-[44px] w-full cursor-pointer items-center gap-2 rounded-lg px-2.5 py-2 text-left text-ui hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                onclick: move |_| on_pick.call(neighbor_idx),
                EntityShapeIcon { kind: neighbor.key.kind }
                span { class: label_cls, "{neighbor.label}" }
                span { class: "shrink-0 text-micro text-subtle tabular-nums",
                    "{w_str} {unit}"
                }
            }
        }
    }
}
