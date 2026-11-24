// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Graph workspace controls bar: history, projection, 2D/3D mode, node finder,
//! camera controls, simulation toggle, and PNG export.

use super::presentation::{
    EntityShapeIcon, criteria_summary, entity_badge_classes, projection_label,
};
use crate::app::routes::Route;
use crate::components::ui::button::Button;
use crate::i18n::{TextKey, format_count, t};
use crate::state::use_app_state_context;
use crate::ui::segmented_control::{SegmentedControl, SegmentedControlItem};
use dioxus::prelude::*;
use lotus_explore_core::filters::ColumnFilters;
use lotus_explore_core::graph::{GraphProjection, NodeKey, NodeMatch, search_all_nodes};
use std::sync::Arc;

/// DOM id of the node-finder search input (targeted by the `/` shortcut).
pub(super) const NODE_FINDER_INPUT_ID: &str = "graph-node-finder";

const PROJECTIONS: [GraphProjection; 4] = [
    GraphProjection::CompoundTaxon,
    GraphProjection::CompoundReference,
    GraphProjection::TaxonReference,
    GraphProjection::Tripartite,
];

/// Control bar above the network canvas and inspector.
#[component]
pub(super) fn GraphToolbar(
    criteria: lotus_model::SearchCriteria,
    filters: ColumnFilters,
    set_arc: Arc<lotus_model::ColumnarResultSet>,
    filtered_rows: Arc<Vec<u32>>,
    effective_projection: GraphProjection,
    is_3d: bool,
    paused: bool,
    can_go_back: bool,
    has_graph: bool,
    on_back: EventHandler<()>,
    on_clear_filters: EventHandler<()>,
    on_projection_change: EventHandler<GraphProjection>,
    on_mode_change: EventHandler<bool>,
    on_focus_node: EventHandler<NodeKey>,
    on_zoom: EventHandler<f32>,
    on_reset_view: EventHandler<()>,
    on_toggle_pause: EventHandler<()>,
    on_export_png: EventHandler<()>,
) -> Element {
    let locale = crate::hooks::use_locale::use_locale();
    let dark_mode = use_app_state_context().state.read().dark_mode;
    let mut finder_query = use_signal(String::new);

    let matches: Vec<NodeMatch> = {
        let q = finder_query.read();
        if q.trim().is_empty() {
            Vec::new()
        } else {
            search_all_nodes(&set_arc, &filtered_rows, &q, 12)
        }
    };

    let pause_label = if paused {
        t(locale, TextKey::GraphResumeMotion)
    } else {
        t(locale, TextKey::GraphPauseMotion)
    };
    let pause_pressed = if paused {
        String::from("true")
    } else {
        String::from("false")
    };
    let mode_value = if is_3d { "3d" } else { "2d" };
    let back_label = format!("← {}", t(locale, TextKey::PreviousSearch));
    let clear_filters_label = t(locale, TextKey::ClearFilters).to_string();
    let table_filters_label = t(locale, TextKey::TableAndFilters).to_string();
    let zoom_in_label = t(locale, TextKey::GraphZoomIn).to_string();
    let zoom_out_label = t(locale, TextKey::GraphZoomOut).to_string();
    let reset_view_label = t(locale, TextKey::GraphResetView).to_string();
    let download_png_label = t(locale, TextKey::GraphDownloadPng).to_string();
    let pause_label_owned = pause_label.to_string();
    let summary = criteria_summary(&criteria);
    let btn_class = "inline-flex shrink-0 items-center justify-center rounded-xl border border-border bg-surface px-3 py-2 text-ui font-semibold text-text shadow-xs hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 disabled:cursor-not-allowed disabled:opacity-50 min-h-[44px] min-w-[44px]";

    rsx! {
        nav {
            aria_label: "{t(locale, TextKey::NetworkControlsNavAria)}",
            class: "flex flex-col gap-2.5 rounded-xl border border-panel-border bg-panel p-3 shadow-xs",
            div { class: "flex flex-wrap items-center gap-2",
                span {
                    class: "inline-flex max-w-[28ch] items-center truncate rounded-full border border-panel-border bg-panel-soft px-2.5 py-1 text-ui font-semibold text-text",
                    "{summary}"
                }
                if filters.is_active() {
                    span { class: "inline-flex items-center gap-1.5 rounded-full border border-stat-reference-border bg-stat-reference px-2.5 py-1 text-ui font-semibold text-text-wd-reference",
                        "{t(locale, TextKey::GraphActiveFilters)}"
                    }
                    Button {
                        r#type: "button",
                        label: clear_filters_label,
                        class: btn_class,
                        onclick: move |_| on_clear_filters.call(()),
                    }
                }
                if can_go_back {
                    Button {
                        r#type: "button",
                        label: back_label,
                        class: btn_class,
                        onclick: move |_| on_back.call(()),
                    }
                }
                Button {
                    r#type: "button",
                    label: table_filters_label,
                    class: btn_class,
                    onclick: move |_| Route::navigate_to_view("search"),
                }

                div { class: "flex max-w-full min-w-0 flex-wrap items-center gap-2 sm:ml-auto",
                    div { class: "flex max-w-full min-w-0 flex-wrap items-center gap-1.5",
                        label {
                            r#for: "graph-projection-select",
                            class: "text-ui font-semibold text-muted",
                            "{t(locale, TextKey::GraphProjectionLabel)}"
                        }
                        select {
                            id: "graph-projection-select",
                            value: "{effective_projection.as_str()}",
                            class: "min-h-[44px] min-w-0 max-w-full cursor-pointer rounded-xl border border-border bg-surface px-2.5 py-1.5 text-body font-semibold text-text shadow-xs focus-visible:border-accent focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                            onchange: move |evt| {
                                if let Some(next) = GraphProjection::parse(&evt.value()) {
                                    on_projection_change.call(next);
                                }
                            },
                            for proj in PROJECTIONS {
                                option {
                                    value: "{proj.as_str()}",
                                    selected: proj == effective_projection,
                                    "{projection_label(locale, proj)}"
                                }
                            }
                        }
                    }

                    SegmentedControl {
                        aria_label: t(locale, TextKey::GraphViewModeAria),
                        selected_value: mode_value,
                        dark: dark_mode,
                        wrap: false,
                        items: vec![
                            SegmentedControlItem {
                                label: t(locale, TextKey::GraphMode3d),
                                value: "3d",
                            },
                            SegmentedControlItem {
                                label: t(locale, TextKey::GraphMode2d),
                                value: "2d",
                            },
                        ],
                        on_select: move |val: String| {
                            on_mode_change.call(val != "2d");
                        },
                    }
                }
            }

            div { class: "flex flex-wrap items-center gap-2",
                div { class: "relative min-w-[220px] flex-1 max-w-sm",
                    label {
                        r#for: NODE_FINDER_INPUT_ID,
                        class: "sr-only",
                        "{t(locale, TextKey::GraphNodeSearchLabel)}"
                    }
                    input {
                        id: NODE_FINDER_INPUT_ID,
                        r#type: "search",
                        value: "{finder_query}",
                        placeholder: "{t(locale, TextKey::GraphNodeSearchPlaceholder)}",
                        class: "min-h-[44px] w-full rounded-xl border border-border bg-surface px-3 py-1.5 text-body text-text shadow-xs placeholder:text-subtle focus-visible:border-accent focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                        oninput: move |evt| finder_query.set(evt.value()),
                    }
                    if !matches.is_empty() {
                        ul {
                            role: "listbox",
                            aria_label: "{t(locale, TextKey::GraphNodeSearchLabel)}",
                            class: "absolute left-0 right-0 top-full z-20 mt-1 max-h-64 overflow-y-auto rounded-xl border border-border bg-panel p-1 shadow-md list-none",
                            for m in matches {
                                FinderResultItem {
                                    item: m,
                                    locale,
                                    on_pick: move |key| {
                                        finder_query.set(String::new());
                                        on_focus_node.call(key);
                                    },
                                }
                            }
                        }
                    }
                }

                div { class: "flex flex-wrap items-center gap-2 ml-auto",
                    Button {
                        r#type: "button",
                        disabled: !has_graph,
                        label: zoom_in_label,
                        class: btn_class,
                        onclick: move |_| on_zoom.call(1.28_f32),
                    }
                    Button {
                        r#type: "button",
                        disabled: !has_graph,
                        label: zoom_out_label,
                        class: btn_class,
                        onclick: move |_| on_zoom.call(0.78_f32),
                    }
                    Button {
                        r#type: "button",
                        disabled: !has_graph,
                        label: reset_view_label,
                        class: btn_class,
                        onclick: move |_| on_reset_view.call(()),
                    }
                    Button {
                        r#type: "button",
                        disabled: !has_graph,
                        label: pause_label_owned,
                        aria_pressed: pause_pressed,
                        class: btn_class,
                        onclick: move |_| on_toggle_pause.call(()),
                    }
                    Button {
                        r#type: "button",
                        disabled: !has_graph,
                        label: download_png_label,
                        class: btn_class,
                        onclick: move |_| on_export_png.call(()),
                    }
                }
            }
        }
    }
}

#[component]
fn FinderResultItem(
    item: NodeMatch,
    locale: crate::i18n::Locale,
    on_pick: EventHandler<NodeKey>,
) -> Element {
    let badge = entity_badge_classes(item.key.kind);
    let label_class = if item.key.kind == lotus_explore_core::graph::NodeKind::Taxon {
        "min-w-0 flex-1 truncate italic text-text"
    } else {
        "min-w-0 flex-1 truncate text-text"
    };
    let occ_text = format_count(locale, usize::try_from(item.occurrence_count).unwrap_or(0));

    rsx! {
        li {
            button {
                r#type: "button",
                class: "flex min-h-[44px] w-full cursor-pointer items-center gap-2 rounded-lg px-2.5 py-2 text-left text-ui hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                onclick: move |_| on_pick.call(item.key),
                EntityShapeIcon { kind: item.key.kind }
                span { class: label_class, "{item.label}" }
                span { class: "shrink-0 font-mono text-micro text-subtle", "{item.qid_text}" }
                span { class: "shrink-0 rounded-full border px-1.5 py-0.5 text-micro font-semibold tabular-nums {badge}",
                    "{occ_text}"
                }
            }
        }
    }
}
