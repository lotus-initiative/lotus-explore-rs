// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! WebGL2 canvas viewport, DOM label layer, dimmed `LotusLoader` pivot overlay,
//! entity shape legend, ego breadcrumb, and honest overview-cap footer.

use super::bridge::{CANVAS_ID, LABELS_ID};
use super::helpers::{
    EntityShapeIcon, corroboration_unit, entity_badge_classes, entity_kind_label,
};
use crate::components::loading::{LotusLoader, query_phase_text, rows_so_far_text};
use crate::components::ui::Button;
use crate::features::explore::selectors::use_lifecycle_selector;
use crate::i18n::{TextKey, aria_graph_kind_toggle, format_count, graph_two_type_note, t};
use crate::state::use_results_context;
use crate::ui::prelude::{NoticeBar, NoticeTone};
use dioxus::prelude::*;
use lotus_explore_core::explore::types::QueryPhase;
use lotus_explore_core::graph::{
    GraphModel, GraphProjection, GraphScope, NodeKind, OVERVIEW_MAX_NODE_PRESETS,
};
use std::sync::Arc;

/// WebGL2 graph canvas stage plus accessible legend and scope/cap controls.
#[component]
pub(super) fn GraphCanvasStage(
    model: Arc<GraphModel>,
    scope: GraphScope,
    dark_mode: bool,
    loading: bool,
    webgl_unavailable: bool,
    context_lost: bool,
    png_notice: Option<String>,
    on_zoom: EventHandler<f32>,
    on_reset_view: EventHandler<()>,
    on_clear_selection: EventHandler<()>,
    on_back_to_overview: EventHandler<()>,
    on_change_max_nodes: EventHandler<usize>,
    on_projection_change: EventHandler<GraphProjection>,
) -> Element {
    let locale = crate::hooks::use_locale();
    let explore = use_results_context().explore;
    let query_phase = *use_lifecycle_selector(explore, |l| l.query_phase).read();
    let rows_so_far = *use_lifecycle_selector(explore, |l| l.rows_so_far).read();

    let node_count = format_count(locale, model.nodes.len());
    let edge_count = format_count(locale, model.edges.len());
    let canvas_aria = format!(
        "{} ({node_count}, {edge_count})",
        t(locale, TextKey::GraphCanvasAriaPrefix)
    );

    let ego_center_label = match scope {
        GraphScope::Ego { .. } => model.nodes.first().map(|node| node.label.clone()),
        GraphScope::Overview { .. } => None,
    };
    let active_limit = match scope {
        GraphScope::Overview { max_nodes } => max_nodes,
        GraphScope::Ego { .. } => 0,
    };

    rsx! {
        div { class: "flex min-w-0 flex-col gap-2.5",
            if webgl_unavailable {
                NoticeBar {
                    label: t(locale, TextKey::Notice).to_string(),
                    tone: NoticeTone::Warning,
                    role: "status",
                    aria_live: "polite",
                    dark: dark_mode,
                    span { class: "flex-1 min-w-0 text-ui text-muted",
                        "{t(locale, TextKey::GraphWebglUnavailable)}"
                    }
                }
            }
            if context_lost {
                NoticeBar {
                    label: t(locale, TextKey::Notice).to_string(),
                    tone: NoticeTone::Warning,
                    role: "status",
                    aria_live: "polite",
                    dark: dark_mode,
                    span { class: "flex-1 min-w-0 text-ui text-muted",
                        "{t(locale, TextKey::GraphContextLost)}"
                    }
                }
            }
            if let Some(saved_path) = png_notice.as_deref() {
                NoticeBar {
                    label: t(locale, TextKey::Notice).to_string(),
                    tone: NoticeTone::Neutral,
                    role: "status",
                    aria_live: "polite",
                    dark: dark_mode,
                    span { class: "flex-1 min-w-0 text-ui text-text break-all",
                        "{t(locale, TextKey::GraphPngSaved)} {saved_path}"
                    }
                }
            }

            div { class: "lotus-graph-stage relative w-full overflow-hidden rounded-xl border border-panel-border bg-surface shadow-xs",
                canvas {
                    id: CANVAS_ID,
                    role: "img",
                    tabindex: "0",
                    aria_label: "{canvas_aria}",
                    class: "lotus-graph-canvas cursor-grab focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                    onkeydown: move |evt| {
                        match evt.key() {
                            Key::Character(ref ch) if ch == "+" || ch == "=" => {
                                evt.prevent_default();
                                on_zoom.call(1.25_f32);
                            }
                            Key::Character(ref ch) if ch == "-" || ch == "_" => {
                                evt.prevent_default();
                                on_zoom.call(0.8_f32);
                            }
                            Key::Character(ref ch) if ch == "0" => {
                                evt.prevent_default();
                                on_reset_view.call(());
                            }
                            Key::Escape => {
                                on_clear_selection.call(());
                            }
                            _ => {}
                        }
                    },
                }

                div {
                    id: LABELS_ID,
                    class: "lotus-graph-labels",
                    aria_hidden: "true",
                }

                if let Some(center_label) = ego_center_label {
                    div { class: "absolute left-3 top-3 z-10 flex flex-wrap items-center gap-2 rounded-xl border border-panel-border bg-glass px-3 py-1.5 shadow-sm backdrop-blur-sm",
                        span { class: "text-ui font-semibold text-text",
                            "{t(locale, TextKey::GraphEgoBadgePrefix)} {center_label}"
                        }
                        Button {
                            r#type: "button",
                            label: t(locale, TextKey::GraphBackToOverview).to_string(),
                            class: "inline-flex min-h-[44px] min-w-[44px] items-center justify-center rounded-lg border border-border bg-surface px-3 py-2 text-ui font-semibold text-text hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                            onclick: move |_| on_back_to_overview.call(()),
                        }
                    }
                }

                if loading {
                    div {
                        role: "status",
                        aria_live: "polite",
                        aria_busy: "true",
                        class: "absolute inset-0 z-10 flex flex-col items-center justify-center gap-3 bg-bg/70 p-6 text-center backdrop-blur-[1px]",
                        LotusLoader {}
                        p { class: "m-0 max-w-[30ch] text-body font-medium text-text",
                            "{query_phase_text(locale, query_phase)}"
                        }
                        if matches!(
                            query_phase,
                            QueryPhase::FetchingResults
                                | QueryPhase::ProcessingResults
                                | QueryPhase::Rendering
                        ) && rows_so_far.is_some()
                        {
                            p { class: "m-0 text-body text-muted tabular-nums",
                                "{rows_so_far_text(locale, rows_so_far.unwrap_or(0))}"
                            }
                        }
                    }
                }
            }

            div { class: "flex flex-col gap-2 rounded-xl border border-panel-border bg-panel px-3 py-2 text-ui",
                div { class: "flex flex-wrap items-center justify-between gap-2",
                    div {
                        role: "group",
                        aria_label: "{t(locale, TextKey::GraphLegendAria)}",
                        class: "flex flex-wrap items-center gap-2",
                        LegendPill {
                            kind: NodeKind::Compound,
                            model: model.clone(),
                            locale,
                            on_pick: on_projection_change,
                        }
                        LegendPill {
                            kind: NodeKind::Taxon,
                            model: model.clone(),
                            locale,
                            on_pick: on_projection_change,
                        }
                        LegendPill {
                            kind: NodeKind::Reference,
                            model: model.clone(),
                            locale,
                            on_pick: on_projection_change,
                        }
                    }
                    p { class: "m-0 text-body text-subtle",
                        "{t(locale, TextKey::GraphCanvasHint)}"
                    }
                }
                if let Some(hidden_kind) = model.effective_projection.omitted_kind()
                    && model.available_kind_count(hidden_kind) > 0
                {
                    p { class: "m-0 text-ui text-muted",
                        "{graph_two_type_note(locale, entity_kind_label(locale, hidden_kind), model.available_kind_count(hidden_kind), &format_count(locale, model.available_kind_count(hidden_kind)), corroboration_unit(locale, model.effective_projection))}"
                    }
                }
            }

            if model.capped && matches!(scope, GraphScope::Overview { .. }) {
                div { class: "flex flex-wrap items-center justify-between gap-2 rounded-xl border border-panel-border bg-panel-soft px-3 py-2 text-ui",
                    span { class: "text-muted tabular-nums",
                        "{t(locale, TextKey::GraphCappedNoticePrefix)} {format_count(locale, model.nodes.len())} / {format_count(locale, model.total_distinct_nodes)} {t(locale, TextKey::GraphCappedNoticeSuffix)}"
                    }
                    div { class: "flex flex-wrap items-center gap-2",
                        span { class: "text-micro font-semibold text-subtle",
                            "{t(locale, TextKey::GraphNodeLimitLabel)}"
                        }
                        for preset in OVERVIEW_MAX_NODE_PRESETS {
                            CapPresetButton {
                                preset,
                                active: active_limit == preset,
                                locale,
                                on_pick: on_change_max_nodes,
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn LegendPill(
    kind: NodeKind,
    model: Arc<GraphModel>,
    locale: crate::i18n::Locale,
    on_pick: EventHandler<GraphProjection>,
) -> Element {
    let active = model.effective_projection.includes_kind(kind);
    let count = if active {
        model.kind_count(kind)
    } else {
        model.available_kind_count(kind)
    };
    let next_proj = GraphProjection::toggle_kind(model.effective_projection, kind)
        .filter(|_| active || count > 0);
    let can_toggle = next_proj.is_some();

    let label = entity_kind_label(locale, kind);
    let count_str = format_count(locale, count);
    let aria_label = aria_graph_kind_toggle(locale, label, &count_str, active, can_toggle);
    let pressed = if active { "true" } else { "false" };

    let style_cls = if active {
        let badge = entity_badge_classes(kind);
        if can_toggle {
            format!("{badge} cursor-pointer hover:opacity-90")
        } else {
            format!("{badge} cursor-default")
        }
    } else if can_toggle {
        String::from(
            "border-dashed border-border bg-surface text-text cursor-pointer hover:bg-panel-soft",
        )
    } else {
        String::from("border-dashed border-border bg-surface text-subtle cursor-default opacity-60")
    };

    rsx! {
        button {
            r#type: "button",
            disabled: !can_toggle,
            aria_pressed: pressed,
            aria_label: "{aria_label}",
            class: "inline-flex min-h-[44px] min-w-[44px] items-center gap-1.5 rounded-full border px-3 py-1.5 text-micro font-semibold tabular-nums focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 {style_cls}",
            onclick: move |_| {
                if let Some(next) = next_proj {
                    on_pick.call(next);
                }
            },
            if !active && can_toggle {
                span { aria_hidden: "true", "+" }
            }
            EntityShapeIcon { kind }
            span { "{label}" }
            if active {
                span { class: "opacity-80", "{count_str}" }
            } else {
                span { "{count_str}" }
            }
        }
    }
}

#[component]
fn CapPresetButton(
    preset: usize,
    active: bool,
    locale: crate::i18n::Locale,
    on_pick: EventHandler<usize>,
) -> Element {
    let label = format_count(locale, preset);
    let class = if active {
        "inline-flex min-h-[44px] min-w-[44px] cursor-pointer items-center justify-center rounded-lg border border-accent bg-primary px-3 py-2 text-ui font-semibold text-white focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2"
    } else {
        "inline-flex min-h-[44px] min-w-[44px] cursor-pointer items-center justify-center rounded-lg border border-border bg-surface px-3 py-2 text-ui font-semibold text-text hover:bg-panel focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2"
    };
    let pressed = if active {
        String::from("true")
    } else {
        String::from("false")
    };
    rsx! {
        Button {
            r#type: "button",
            label,
            aria_pressed: pressed,
            class,
            onclick: move |_| on_pick.call(preset),
        }
    }
}
