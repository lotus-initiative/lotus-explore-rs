// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Interactive WebGL2 network graph workspace (`/network`).
//!
//! Reads directly from `AppRoot`'s shared `ExploreState`, `ColumnFilters`, and
//! `SearchCriteria` with zero extra SPARQL calls on mount, inspection, search,
//! or 1-hop ego focusing.

use crate::app::routes::Route;
use crate::components::layout::notices::{ErrorNotice, LookupNotice};
use crate::components::loading::LoadingState;
use crate::components::ui::Button;
use crate::features::explore::ExploreUiState;
use crate::features::explore::interactions::{ExploreInteractions, use_explore_interactions};
use crate::features::explore::selectors::{
    use_lifecycle_selector, use_result_arc_selector, use_result_selector,
};
use crate::i18n::{TextKey, t};
use crate::state::{
    FormCriteriaContext, use_app_state_context, use_form_criteria_context, use_results_context,
};
use crate::ui::{ContentPhase, LifecycleBooleans};
use dioxus::prelude::*;
use lotus_explore_core::filters::ColumnFilters;
use lotus_explore_core::graph::{
    DEFAULT_EGO_MAX_NEIGHBORS, GraphEmptyReason, GraphModel, GraphProjection, GraphScope, NodeKey,
    NodeKind, build_graph, pivot_criteria_for_node,
};
use std::sync::Arc;

mod bridge;
mod canvas_stage;
mod empty_starter;
mod helpers;
mod history;
mod inspector;
mod toolbar;

use bridge::{CANVAS_ID, GraphEvent, LABELS_ID};
use canvas_stage::GraphCanvasStage;
use empty_starter::NetworkEmptyStarter;
use history::PivotHistory;
use inspector::GraphInspector;
use toolbar::{GraphToolbar, NODE_FINDER_INPUT_ID};

/// Full interactive network workspace mounted inside [`crate::pages::NetworkPage`].
#[component]
pub fn ResultsGraphWorkspace() -> Element {
    let locale = crate::hooks::use_locale();
    let app_state = use_app_state_context().state;
    let dark_mode = app_state.read().dark_mode;
    let explore = use_results_context().explore;
    let form = use_form_criteria_context();
    let interactions = use_explore_interactions();

    let script_ready = use_signal(|| false);
    let gl_ready = use_signal(|| false);
    let mounted = use_signal(|| false);
    let webgl_unavailable = use_signal(|| false);
    let context_lost = use_signal(|| false);
    let png_notice = use_signal(|| Option::<String>::None);
    let mut projection = use_signal(GraphProjection::default);
    let mut scope = use_signal(GraphScope::default);
    let mut is_3d = use_signal(|| true);
    let mut paused = use_signal(|| false);
    let selected_idx = use_signal(|| Option::<usize>::None);
    let last_selected_key = use_signal(|| Option::<NodeKey>::None);
    let had_rendered_graph = use_signal(|| false);
    let mut pivot_history = use_signal(PivotHistory::new);

    let ui_state = use_memo(move || ExploreUiState::from_explore(explore, form.is_dirty()));
    let set_arc = use_result_arc_selector(explore, |r| r.set.clone());
    let filters = use_result_selector(explore, |r| r.filters.clone());
    let searched_once_sig = use_lifecycle_selector(explore, |l| l.searched_once);

    let filtered_rows = use_memo(move || {
        let set_guard = set_arc.read();
        let set = &set_guard.0;
        let active_filters = filters.read();
        let rows = if active_filters.is_active() {
            set.plan_filter(&active_filters.to_spec())
                .surviving_rows(set)
        } else {
            (0..set.row_count())
                .filter_map(|idx| u32::try_from(idx).ok())
                .collect()
        };
        Arc::new(rows)
    });

    let graph_model = use_memo(move || {
        let set_guard = set_arc.read();
        let rows = filtered_rows.read();
        let model = build_graph(&set_guard.0, &rows, *projection.read(), *scope.read());
        Arc::new(model)
    });

    let ui_snap = *ui_state.read();
    let phase = ContentPhase::from(LifecycleBooleans {
        loading: ui_snap.loading,
        has_error: ui_snap.has_error,
        searched_once: ui_snap.searched_once,
        download_only_mode: ui_snap.download_only_mode,
        has_entries: ui_snap.has_entries,
        criteria_dirty: ui_snap.criteria_dirty,
    });
    let current_model = graph_model.read().clone();
    let has_renderable_graph =
        current_model.empty_reason.is_none() && !current_model.nodes.is_empty();
    let searched_ever = *searched_once_sig.read() || ui_snap.searched_once;
    let is_loading = ui_snap.loading;
    let show_canvas = has_renderable_graph || (is_loading && *had_rendered_graph.read());

    setup_graph_effects(&GraphBridgeEffects {
        app_state,
        ui_state,
        graph_model,
        scope,
        is_3d,
        paused,
        script_ready,
        gl_ready,
        mounted,
        webgl_unavailable,
        context_lost,
        png_notice,
        selected_idx,
        last_selected_key,
        had_rendered_graph,
    });

    let runner = SearchRunnerSignals {
        form,
        pivot_history,
        selected_idx,
        last_selected_key,
        scope,
    };
    let run_interactions = interactions.clone();
    let back_interactions = interactions.clone();
    let clear_filters_interactions = interactions.clone();
    let empty_clear_interactions = interactions.clone();
    let pivot_interactions = interactions.clone();
    let filter_table_interactions = interactions;

    rsx! {
        section {
            role: "region",
            aria_label: "{t(locale, TextKey::NetworkWorkspaceRegionAria)}",
            class: "flex w-full flex-col gap-3",
            onkeydown: move |evt| {
                if let Key::Character(ref ch) = evt.key()
                    && ch == "/"
                {
                    bridge::focus_finder_if_not_typing(NODE_FINDER_INPUT_ID);
                }
            },
            ErrorNotice {}
            LookupNotice {}

            if !searched_ever && !is_loading && !ui_snap.has_entries {
                NetworkEmptyStarter {
                    on_run: move |criteria| {
                        run_search_criteria(criteria, false, &run_interactions, runner);
                    },
                }
            } else if is_loading && !show_canvas {
                LoadingState {}
            } else {
                GraphToolbar {
                    criteria: form.criteria.read().clone(),
                    filters: filters.read().clone(),
                    set_arc: set_arc.read().0.clone(),
                    filtered_rows: filtered_rows.read().clone(),
                    projection: *projection.read(),
                    is_3d: *is_3d.read(),
                    paused: *paused.read(),
                    can_go_back: !pivot_history.read().is_empty(),
                    has_graph: has_renderable_graph,
                    on_back: move |()| {
                        let popped = pivot_history.write().pop();
                        if let Some(prev) = popped {
                            run_search_criteria(prev, false, &back_interactions, runner);
                        }
                    },
                    on_clear_filters: move |()| clear_filters_interactions.clear_filters(),
                    on_projection_change: move |next| projection.set(next),
                    on_mode_change: move |next_3d| is_3d.set(next_3d),
                    on_focus_node: move |key| focus_ego_scope(key, selected_idx, last_selected_key, scope),
                    on_zoom: bridge::zoom_graph,
                    on_reset_view: move |()| bridge::reset_graph_view(),
                    on_toggle_pause: move |()| {
                        let next = !*paused.peek();
                        paused.set(next);
                    },
                    on_export_png: move |()| bridge::trigger_export_png(),
                }

                if matches!(phase, ContentPhase::Stale) {
                    div { class: "rounded-xl border border-panel-border bg-panel-soft px-3 py-2 text-ui text-muted",
                        "{t(locale, TextKey::StaleResults)}"
                    }
                }

                if show_canvas {
                    div { class: "grid grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_360px]",
                        GraphCanvasStage {
                            model: current_model.clone(),
                            scope: *scope.read(),
                            dark_mode,
                            loading: is_loading,
                            webgl_unavailable: *webgl_unavailable.read(),
                            context_lost: *context_lost.read(),
                            png_notice: png_notice.read().clone(),
                            on_zoom: bridge::zoom_graph,
                            on_reset_view: move |()| bridge::reset_graph_view(),
                            on_clear_selection: move |()| {
                                select_node_from_ui(None, graph_model, selected_idx, last_selected_key);
                            },
                            on_back_to_overview: move |()| scope.set(GraphScope::default()),
                            on_change_max_nodes: move |max_nodes| {
                                scope.set(GraphScope::Overview { max_nodes });
                            },
                        }
                        GraphInspector {
                            model: current_model,
                            set_arc: set_arc.read().0.clone(),
                            scope: *scope.read(),
                            selected_idx: *selected_idx.read(),
                            on_select: move |idx| {
                                select_node_from_ui(idx, graph_model, selected_idx, last_selected_key);
                            },
                            on_focus_ego: move |key| {
                                focus_ego_scope(key, selected_idx, last_selected_key, scope);
                            },
                            on_back_to_overview: move |()| scope.set(GraphScope::default()),
                            on_filter_table: move |new_filters: ColumnFilters| {
                                filter_table_interactions.set_filters(new_filters);
                                Route::navigate_to_view("search");
                            },
                            on_pivot: move |(kind, qid): (NodeKind, String)| {
                                let next =
                                    pivot_criteria_for_node(kind, &qid, crate::clock::current_year());
                                run_search_criteria(next, true, &pivot_interactions, runner);
                            },
                        }
                    }
                } else if !ui_snap.has_error {
                    GraphEmptyReasonNotice {
                        reason: current_model.empty_reason,
                        locale,
                        has_active_filters: filters.read().is_active(),
                        on_back_to_overview: move |()| scope.set(GraphScope::default()),
                        on_clear_filters: move |()| empty_clear_interactions.clear_filters(),
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
struct SearchRunnerSignals {
    form: FormCriteriaContext,
    pivot_history: Signal<PivotHistory>,
    selected_idx: Signal<Option<usize>>,
    last_selected_key: Signal<Option<NodeKey>>,
    scope: Signal<GraphScope>,
}

fn run_search_criteria(
    new_criteria: lotus_model::SearchCriteria,
    push_history: bool,
    interactions: &ExploreInteractions,
    mut runner: SearchRunnerSignals,
) {
    if push_history {
        let prev = runner.form.criteria.peek().clone();
        runner.pivot_history.write().push(prev);
    }
    runner.selected_idx.set(None);
    runner.last_selected_key.set(None);
    runner.scope.set(GraphScope::default());
    let mut criteria_sig = runner.form.criteria;
    *criteria_sig.write() = new_criteria;
    interactions.search();
}

fn select_node_from_ui(
    idx: Option<usize>,
    graph_model: Memo<Arc<GraphModel>>,
    mut selected_idx: Signal<Option<usize>>,
    mut last_selected_key: Signal<Option<NodeKey>>,
) {
    selected_idx.set(idx);
    let key = idx.and_then(|i| graph_model.peek().nodes.get(i).map(|n| n.key));
    last_selected_key.set(key);
    bridge::push_select_node(idx);
}

fn focus_ego_scope(
    key: NodeKey,
    mut selected_idx: Signal<Option<usize>>,
    mut last_selected_key: Signal<Option<NodeKey>>,
    mut scope: Signal<GraphScope>,
) {
    selected_idx.set(Some(0));
    last_selected_key.set(Some(key));
    scope.set(GraphScope::Ego {
        center: key,
        max_neighbors: DEFAULT_EGO_MAX_NEIGHBORS,
    });
}

#[derive(Clone, Copy)]
struct GraphBridgeEffects {
    app_state: Signal<crate::app_state::AppState>,
    ui_state: Memo<ExploreUiState>,
    graph_model: Memo<Arc<GraphModel>>,
    scope: Signal<GraphScope>,
    is_3d: Signal<bool>,
    paused: Signal<bool>,
    script_ready: Signal<bool>,
    gl_ready: Signal<bool>,
    mounted: Signal<bool>,
    webgl_unavailable: Signal<bool>,
    context_lost: Signal<bool>,
    png_notice: Signal<Option<String>>,
    selected_idx: Signal<Option<usize>>,
    last_selected_key: Signal<Option<NodeKey>>,
    had_rendered_graph: Signal<bool>,
}

fn setup_graph_effects(ctx: &GraphBridgeEffects) {
    attach_bridge_listener(ctx);
    attach_bridge_mount_and_sync(ctx);
    use_drop(bridge::destroy_graph);
}

fn attach_bridge_listener(ctx: &GraphBridgeEffects) {
    let GraphBridgeEffects {
        graph_model,
        mut script_ready,
        mut gl_ready,
        mut webgl_unavailable,
        mut context_lost,
        png_notice,
        mut selected_idx,
        mut last_selected_key,
        ..
    } = *ctx;
    use_effect(move || {
        spawn(async move {
            let mut eval = document::eval(bridge::listener_script());
            while let Ok(message) = eval.recv::<serde_json::Value>().await {
                let Some(event) = bridge::parse_graph_event(&message) else {
                    continue;
                };
                match event {
                    GraphEvent::ScriptLoaded => script_ready.set(true),
                    GraphEvent::Ready => {
                        gl_ready.set(true);
                        webgl_unavailable.set(false);
                        context_lost.set(false);
                    }
                    GraphEvent::Unavailable => {
                        webgl_unavailable.set(true);
                        gl_ready.set(false);
                    }
                    GraphEvent::Select(idx) => {
                        selected_idx.set(idx);
                        let key = idx.and_then(|i| graph_model.peek().nodes.get(i).map(|n| n.key));
                        last_selected_key.set(key);
                    }
                    GraphEvent::Hover(_) | GraphEvent::Settled => {}
                    GraphEvent::ContextLost => context_lost.set(true),
                    GraphEvent::ContextRestored => context_lost.set(false),
                    GraphEvent::PngData(data_url) => {
                        handle_png_data(&data_url, png_notice);
                    }
                }
            }
        });
    });
}

fn attach_bridge_mount_and_sync(ctx: &GraphBridgeEffects) {
    let GraphBridgeEffects {
        app_state,
        ui_state,
        graph_model,
        scope,
        is_3d,
        paused,
        script_ready,
        mut gl_ready,
        mut mounted,
        mut selected_idx,
        last_selected_key,
        mut had_rendered_graph,
        ..
    } = *ctx;
    use_effect(move || {
        let model = graph_model.read();
        let has_data = model.empty_reason.is_none() && !model.nodes.is_empty();
        if has_data && !*had_rendered_graph.peek() {
            had_rendered_graph.set(true);
        }
        let show_canvas = has_data || (ui_state.read().loading && *had_rendered_graph.read());
        let can_mount = show_canvas && *script_ready.read();
        if can_mount {
            if !*mounted.peek() {
                bridge::mount_graph(CANVAS_ID, LABELS_ID, cfg!(not(target_arch = "wasm32")));
                mounted.set(true);
            }
        } else if *mounted.peek() {
            bridge::destroy_graph();
            mounted.set(false);
            gl_ready.set(false);
        }
    });

    use_effect(move || {
        if !*gl_ready.read() {
            return;
        }
        bridge::push_theme(app_state.read().dark_mode);
    });

    use_effect(move || {
        if !*gl_ready.read() {
            return;
        }
        bridge::push_mode(*is_3d.read());
    });

    use_effect(move || {
        if !*gl_ready.read() {
            return;
        }
        bridge::push_loading(ui_state.read().loading);
    });

    use_effect(move || {
        if !*gl_ready.read() {
            return;
        }
        bridge::push_paused(*paused.read());
    });

    use_effect(move || {
        let model = graph_model.read();
        let has_data = model.empty_reason.is_none() && !model.nodes.is_empty();
        if !*gl_ready.read() || !has_data {
            return;
        }
        let payload = model.to_payload();
        bridge::push_graph_payload(&payload);
        let next_sel = if matches!(*scope.peek(), GraphScope::Ego { .. }) {
            Some(0)
        } else if let Some(prev_key) = *last_selected_key.peek() {
            model.index_of(prev_key)
        } else {
            None
        };
        if *selected_idx.peek() != next_sel {
            selected_idx.set(next_sel);
        }
        bridge::push_select_node(next_sel);
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn handle_png_data(data_url: &str, mut png_notice: Signal<Option<String>>) {
    if let Ok(path) = crate::download::save_png_data_url(data_url, "lotus-network.png") {
        png_notice.set(Some(path.display().to_string()));
    }
}

#[cfg(target_arch = "wasm32")]
fn handle_png_data(_data_url: &str, _png_notice: Signal<Option<String>>) {}

#[component]
fn GraphEmptyReasonNotice(
    reason: Option<GraphEmptyReason>,
    locale: crate::i18n::Locale,
    has_active_filters: bool,
    on_back_to_overview: EventHandler<()>,
    on_clear_filters: EventHandler<()>,
) -> Element {
    let msg = match reason {
        Some(GraphEmptyReason::NoOccurrenceEdges) => t(locale, TextKey::GraphNoOccurrenceEdges),
        Some(GraphEmptyReason::CenterNotInRows) => t(locale, TextKey::GraphCenterNotInRows),
        Some(GraphEmptyReason::NoRows) | None => t(locale, TextKey::NoResults),
    };
    let btn_class = "inline-flex items-center justify-center rounded-xl border border-border bg-surface px-3.5 py-2 text-ui font-semibold text-text shadow-xs hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2 min-h-[44px] min-w-[44px]";

    rsx! {
        div {
            role: "status",
            aria_live: "polite",
            class: "flex flex-col items-center justify-center gap-3 rounded-xl border border-panel-border bg-panel p-8 text-center",
            p { class: "m-0 max-w-[54ch] text-body text-muted", "{msg}" }
            div { class: "flex flex-wrap items-center justify-center gap-2",
                if matches!(reason, Some(GraphEmptyReason::CenterNotInRows)) {
                    Button {
                        r#type: "button",
                        label: t(locale, TextKey::GraphBackToOverview).to_string(),
                        class: btn_class,
                        onclick: move |_| on_back_to_overview.call(()),
                    }
                }
                if matches!(reason, Some(GraphEmptyReason::NoOccurrenceEdges)) {
                    Button {
                        r#type: "button",
                        label: t(locale, TextKey::TableAndFilters).to_string(),
                        class: btn_class,
                        onclick: move |_| Route::navigate_to_view("search"),
                    }
                }
                if has_active_filters {
                    Button {
                        r#type: "button",
                        label: t(locale, TextKey::ClearFilters).to_string(),
                        class: btn_class,
                        onclick: move |_| on_clear_filters.call(()),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
