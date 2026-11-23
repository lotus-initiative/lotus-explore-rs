// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit and SSR accessibility/contract tests for `results_graph` and `/search` bridge.

#![allow(clippy::expect_used, clippy::panic)]

use super::bridge::{GraphEvent, parse_graph_event};
use super::canvas_stage::GraphCanvasStage;
use super::empty_starter::quick_search_criteria;
use super::history::{MAX_PIVOT_HISTORY, PivotHistory};
use super::inspector::GraphInspector;
use super::*;
use crate::app_state::AppState;
use crate::components::results_table::ResultsTable;
use crate::features::explore::interactions::ExploreInteractions;
use crate::features::explore::orchestrator::SearchTaskController;
use crate::features::explore::{GraphViewParams, parse_graph_view_from_params};
use crate::i18n::{Locale, TextKey, t};
use crate::repositories::HybridRepository;
use crate::state::{AppStateContext, FormCriteriaContext, ResultsContext};
use lotus_explore_core::explore::actions::ExploreAction;
use lotus_explore_core::explore::command::SearchCommand;
use lotus_explore_core::explore::search_state::{ExploreState, reduce_mut};
use lotus_explore_core::explore::url_codec::QueryParams;
use lotus_explore_core::export::metadata::SparqlEndpoint;
use lotus_explore_core::graph::{
    DEFAULT_MAX_NODES, GraphEmptyReason, GraphModel, GraphProjection, GraphScope, NodeKind,
    build_graph,
};
use lotus_model::{ColumnarResultSet, CompoundEntry, SearchCriteria};
use std::sync::Arc;

fn entry_row(
    compound: (&str, &str),
    taxon: (&str, &str),
    reference: (&str, &str),
) -> CompoundEntry {
    CompoundEntry {
        compound_qid: Arc::from(compound.0),
        name: Arc::from(compound.1),
        inchikey: Some(Arc::from("AAAAAAAAAAAAAA-BBBBBBBBBB-C")),
        smiles: Some(Arc::from("CC1=CC(=O)OC1")),
        mass: Some(164.08),
        formula: Some(Arc::from("C10H12O2")),
        taxon_qid: Arc::from(taxon.0),
        taxon_name: Arc::from(taxon.1),
        reference_qid: Arc::from(reference.0),
        reference_node: Arc::from(""),
        ref_title: (!reference.1.is_empty()).then(|| Arc::from(reference.1)),
        ref_doi: (!reference.0.is_empty()).then(|| Arc::from("10.1000/lotus")),
        pub_year: (!reference.0.is_empty()).then_some(2023),
        statement: None,
    }
}

fn sample_result_set(count: usize) -> ColumnarResultSet {
    let mut rows = Vec::with_capacity(count);
    for idx in 0..count {
        let c_qid = format!("Q{}", 100 + idx);
        let t_qid = format!("Q{}", 2000 + (idx % 3));
        let r_qid = format!("Q{}", 5000 + (idx % 2));
        rows.push(entry_row(
            (&c_qid, &format!("Compound-{idx}")),
            (&t_qid, &format!("Gentiana sp{idx}")),
            (&r_qid, &format!("Paper-{idx}")),
        ));
    }
    ColumnarResultSet::from_entries(&rows)
}

fn render_dom<P: Clone + 'static>(component: fn(P) -> Element, props: P) -> String {
    let mut dom = VirtualDom::new_with_props(component, props);
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[derive(Clone, PartialEq)]
struct WorkspaceHarnessProps {
    set: Arc<ColumnarResultSet>,
    searched: bool,
    criteria: SearchCriteria,
    initial_view: Option<GraphViewParams>,
}

impl WorkspaceHarnessProps {
    fn simple(set: Arc<ColumnarResultSet>, searched: bool) -> Self {
        Self {
            set,
            searched,
            criteria: SearchCriteria::up_to_year(2026),
            initial_view: None,
        }
    }
}

fn seeded_explore_state(set: &Arc<ColumnarResultSet>, searched: bool) -> ExploreState {
    let mut initial = ExploreState::default();
    if searched {
        reduce_mut(
            &mut initial,
            ExploreAction::SearchRequested {
                criteria_snapshot: SearchCriteria::up_to_year(2026),
                command: SearchCommand::Interactive,
            },
        );
        reduce_mut(
            &mut initial,
            ExploreAction::SearchSucceeded {
                set: Arc::clone(set),
                qid: None,
                warnings: vec![],
                query: String::from("SELECT * WHERE {}"),
                display_capped_rows: false,
                query_hash: Arc::from("qh"),
                result_hash: Arc::from("rh"),
                metadata_json: Arc::from("{}"),
                endpoint: SparqlEndpoint::Qlever,
            },
        );
    }
    initial
}

#[component]
fn WorkspaceHarness(props: WorkspaceHarnessProps) -> Element {
    let locale_sig = use_signal(|| Locale::En);
    let app_state_sig = use_signal(AppState::default);
    let explore = use_signal(|| seeded_explore_state(&props.set, props.searched));
    let criteria = use_signal(|| props.criteria.clone());
    let baseline = use_signal(|| props.criteria.clone());
    let form = FormCriteriaContext::new(criteria, baseline);
    let interactions = ExploreInteractions::new(
        criteria,
        form,
        explore,
        SearchTaskController::new(),
        HybridRepository,
    );
    use_context_provider(|| locale_sig);
    use_context_provider(|| AppStateContext {
        state: app_state_sig,
    });
    use_context_provider(|| ResultsContext::new(explore));
    use_context_provider(|| form);
    use_context_provider(|| interactions);
    rsx! { ResultsGraphWorkspace { initial_view: props.initial_view } }
}

#[component]
fn TableHarness(props: WorkspaceHarnessProps) -> Element {
    let locale_sig = use_signal(|| Locale::En);
    let app_state_sig = use_signal(AppState::default);
    let explore = use_signal(|| seeded_explore_state(&props.set, props.searched));
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
    use_context_provider(|| locale_sig);
    use_context_provider(|| AppStateContext {
        state: app_state_sig,
    });
    use_context_provider(|| ResultsContext::new(explore));
    use_context_provider(|| form);
    use_context_provider(|| interactions);
    rsx! { ResultsTable {} }
}

#[derive(Clone, PartialEq)]
struct SelectedInspectorHarnessProps {
    model: Arc<GraphModel>,
    set: Arc<ColumnarResultSet>,
    scope: GraphScope,
    selected: Option<usize>,
}

#[component]
fn StageAndInspectorHarness(props: SelectedInspectorHarnessProps) -> Element {
    let locale_sig = use_signal(|| Locale::En);
    let app_state_sig = use_signal(AppState::default);
    let explore = use_signal(ExploreState::default);
    use_context_provider(|| locale_sig);
    use_context_provider(|| AppStateContext {
        state: app_state_sig,
    });
    use_context_provider(|| ResultsContext::new(explore));

    rsx! {
        div {
            GraphCanvasStage {
                model: props.model.clone(),
                scope: props.scope,
                dark_mode: false,
                loading: false,
                webgl_unavailable: false,
                context_lost: false,
                png_notice: None,
                on_zoom: |_| {},
                on_reset_view: move |()| {},
                on_clear_selection: move |()| {},
                on_back_to_overview: move |()| {},
                on_change_max_nodes: move |_| {},
                on_projection_change: move |_| {},
            }
            GraphInspector {
                model: props.model,
                set_arc: props.set,
                scope: props.scope,
                selected_idx: props.selected,
                on_select: move |_| {},
                on_focus_ego: move |_| {},
                on_back_to_overview: move |()| {},
                on_filter_table: move |_| {},
                on_pivot: move |_| {},
            }
        }
    }
}

#[test]
fn pivot_history_is_bounded_and_lifo() {
    let mut history = PivotHistory::new();
    assert!(history.is_empty());

    for idx in 0..(MAX_PIVOT_HISTORY + 5) {
        let mut c = SearchCriteria::up_to_year(2026);
        c.taxon = format!("Taxon-{idx}");
        history.push(c);
    }
    assert_eq!(history.len(), MAX_PIVOT_HISTORY);
    assert_eq!(history.pop().expect("top").taxon, "Taxon-24");
    assert_eq!(history.pop().expect("next").taxon, "Taxon-23");
}

#[test]
fn quick_search_criteria_maps_kinds_and_rejects_blank() {
    assert!(quick_search_criteria(NodeKind::Taxon, "   ", 2026).is_none());

    let c_tax = quick_search_criteria(NodeKind::Taxon, " Gentiana ", 2026).expect("taxon");
    assert_eq!(c_tax.taxon, "Gentiana");

    let c_comp = quick_search_criteria(NodeKind::Compound, "Q121802", 2026).expect("compound");
    assert_eq!(c_comp.structure, "Q121802");
    assert_eq!(c_comp.taxon, "*");

    let c_ref = quick_search_criteria(NodeKind::Reference, "10.1000/abc", 2026).expect("reference");
    assert_eq!(c_ref.reference, "10.1000/abc");
}

#[test]
fn parse_graph_event_decodes_all_bridge_variants() {
    assert_eq!(
        parse_graph_event(&serde_json::json!({ "type": "script_loaded" })),
        Some(GraphEvent::ScriptLoaded)
    );
    assert_eq!(
        parse_graph_event(&serde_json::json!({ "type": "ready" })),
        Some(GraphEvent::Ready)
    );
    assert_eq!(
        parse_graph_event(&serde_json::json!({ "type": "unavailable" })),
        Some(GraphEvent::Unavailable)
    );
    assert_eq!(
        parse_graph_event(&serde_json::json!({ "type": "select", "index": 4 })),
        Some(GraphEvent::Select(Some(4)))
    );
    assert_eq!(
        parse_graph_event(&serde_json::json!({ "type": "select", "index": null })),
        Some(GraphEvent::Select(None))
    );
    assert_eq!(
        parse_graph_event(
            &serde_json::json!({ "type": "png_data", "dataUrl": "data:image/png;base64,AA==" })
        ),
        Some(GraphEvent::PngData(String::from(
            "data:image/png;base64,AA=="
        )))
    );
    assert_eq!(
        parse_graph_event(&serde_json::json!({ "type": "other" })),
        None
    );
}

#[test]
fn bare_network_workspace_renders_starter_cards_without_canvas_or_auto_query() {
    let html = render_dom(
        WorkspaceHarness,
        WorkspaceHarnessProps::simple(Arc::new(ColumnarResultSet::default()), false),
    );
    assert!(html.contains(t(Locale::En, TextKey::NetworkStarterHeading)));
    assert!(html.contains(t(Locale::En, TextKey::NetworkStarterTitleBetaSitosterol)));
    assert!(html.contains(t(Locale::En, TextKey::NetworkStarterTitleGentianaLutea)));
    assert!(
        html.contains("Open advanced search") && html.contains("filters"),
        "expected OpenAdvancedSearch button in:\n{html}"
    );
    assert!(!html.contains(CANVAS_ID));
}

fn strip_html_tags(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn assert_no_redundant_titles(html: &str) {
    let mut rest = html;
    while let Some((before, after_attr)) = rest.split_once(" title=\"") {
        let Some((title_val, after_quote)) = after_attr.split_once('"') else {
            break;
        };
        let tag_open = before.rsplit_once('<').map_or(before, |(_, tail)| tail);
        let tag_name = tag_open.split_whitespace().next().unwrap_or("");
        if let Some((_, inner_rest)) = after_quote.split_once('>') {
            let close_tag = format!("</{tag_name}>");
            if let Some((inner, _)) = inner_rest.split_once(&close_tag) {
                let text = strip_html_tags(inner);
                assert!(
                    !text.trim().eq_ignore_ascii_case(title_val.trim()),
                    "redundant title=\"{title_val}\" matches element text \"{text}\""
                );
            }
        }
        rest = after_quote;
    }
}

#[test]
fn loaded_network_workspace_exposes_accessible_canvas_controls_and_top_hubs() {
    let set = Arc::new(sample_result_set(4));
    let html = render_dom(WorkspaceHarness, WorkspaceHarnessProps::simple(set, true));
    assert!(html.contains(r#"id="lotus-network-canvas""#));
    assert!(html.contains("lotus-graph-canvas"));
    assert!(html.contains(r#"role="img""#));
    assert!(html.contains(r#"tabindex="0""#));
    assert!(html.contains(r#"id="lotus-network-labels""#));
    assert!(html.contains(t(Locale::En, TextKey::GraphZoomIn)));
    assert!(html.contains(t(Locale::En, TextKey::GraphZoomOut)));
    assert!(html.contains(t(Locale::En, TextKey::GraphResetView)));
    assert!(html.contains(t(Locale::En, TextKey::GraphDownloadPng)));
    assert!(html.contains(t(Locale::En, TextKey::TopConnectedEntities)));
    assert!(html.contains(r#"id="graph-projection-select""#));
    assert!(html.contains("min-w-0 max-w-full"));
    assert!(!html.contains(r#"title="*""#));
    assert_no_redundant_titles(&html);
    assert!(
        !html.contains("Auto (") && !html.contains(r#"value="auto""#),
        "duplicated Auto option must be removed from graph view selector:\n{html}"
    );
    for proj_val in [
        "compound_taxon",
        "compound_reference",
        "taxon_reference",
        "tripartite",
    ] {
        assert!(
            html.contains(&format!(r#"value="{proj_val}""#)),
            "missing projection option {proj_val} in:\n{html}"
        );
    }
    let (_, after_select) = html
        .split_once(r#"id="graph-projection-select""#)
        .expect("projection select element");
    let (select_html, _) = after_select
        .split_once("</select>")
        .expect("closing select");
    assert_eq!(
        select_html
            .matches(t(Locale::En, TextKey::ProjectionCompoundTaxon))
            .count(),
        1,
        "Compounds ↔ Taxa must appear once in selector, not duplicated with Auto:\n{select_html}"
    );
    assert!(
        html.contains(r#"aria-pressed="true""#) && html.contains(r#"aria-pressed="false""#),
        "legend must expose interactive active/hidden entity toggles:\n{html}"
    );
    assert!(
        html.contains("Reference</span><span>2</span>"),
        "hidden Reference pill must show available count (2) at full contrast rather than 0:\n{html}"
    );
    assert!(
        html.contains("Showing 2 types for clarity (2 refs weight the links)"),
        "legend must explain why 2 types are shown and how to add the 3rd:\n{html}"
    );
}

#[test]
fn inspector_renders_selected_compound_cdk_depict_and_cap_and_ego_controls() {
    let set = Arc::new(sample_result_set(5));
    let rows: Vec<u32> = (0..5).collect();
    let overview = Arc::new(build_graph(
        &set,
        &rows,
        GraphProjection::Auto,
        GraphScope::Overview { max_nodes: 3 },
    ));
    assert!(overview.capped);

    let compound_idx = overview
        .nodes
        .iter()
        .position(|n| n.key.kind == NodeKind::Compound)
        .expect("compound in capped graph");
    let html_overview = render_dom(
        StageAndInspectorHarness,
        SelectedInspectorHarnessProps {
            model: overview.clone(),
            set: set.clone(),
            scope: GraphScope::Overview { max_nodes: 3 },
            selected: Some(compound_idx),
        },
    );
    assert!(html_overview.contains("https://www.simolecule.com/cdkdepict/depict/cow/svg?smi="));
    assert!(html_overview.contains("annotate=cip"));
    assert!(html_overview.contains(t(Locale::En, TextKey::CopySmiles)));
    assert!(!html_overview.contains(r#"title="Copy SMILES""#));
    assert!(!html_overview.contains(r#"aria-label="Copy SMILES""#));
    assert_no_redundant_titles(&html_overview);
    assert!(html_overview.contains(t(Locale::En, TextKey::ExploreInLotus)));
    assert!(html_overview.contains(t(Locale::En, TextKey::GraphCappedNoticePrefix)));
    assert!(html_overview.contains(t(Locale::En, TextKey::ProjectionCompoundTaxon)));
    assert!(html_overview.contains("1,000"));
    assert!(html_overview.contains("5,000"));
    assert!(html_overview.contains("10,000"));

    let center_key = overview.nodes.first().expect("center").key;
    let ego = Arc::new(build_graph(
        &set,
        &rows,
        GraphProjection::Auto,
        GraphScope::Ego {
            center: center_key,
            max_neighbors: 100,
        },
    ));
    let html_ego = render_dom(
        StageAndInspectorHarness,
        SelectedInspectorHarnessProps {
            model: ego,
            set,
            scope: GraphScope::Ego {
                center: center_key,
                max_neighbors: 100,
            },
            selected: Some(0),
        },
    );
    assert!(html_ego.contains(t(Locale::En, TextKey::GraphBackToOverview)));
    assert!(html_ego.contains(t(Locale::En, TextKey::GraphEgoBadgePrefix)));
    assert!(html_ego.contains(t(Locale::En, TextKey::ProjectionCompoundTaxon)));
}

#[test]
fn structure_only_rows_without_occurrences_show_honest_empty_reason() {
    let raw = ColumnarResultSet::from_entries(&[CompoundEntry {
        compound_qid: Arc::from("Q121802"),
        name: Arc::from("beta-Sitosterol"),
        inchikey: None,
        smiles: Some(Arc::from("CC")),
        mass: Some(414.39),
        formula: Some(Arc::from("C29H50O")),
        taxon_qid: Arc::from(""),
        taxon_name: Arc::from(""),
        reference_qid: Arc::from(""),
        reference_node: Arc::from(""),
        ref_title: None,
        ref_doi: None,
        pub_year: None,
        statement: None,
    }]);
    let set = Arc::new(raw);
    let model = build_graph(&set, &[0], GraphProjection::Auto, GraphScope::default());
    assert_eq!(
        model.empty_reason,
        Some(GraphEmptyReason::NoOccurrenceEdges)
    );

    let html = render_dom(WorkspaceHarness, WorkspaceHarnessProps::simple(set, true));
    assert!(html.contains(t(Locale::En, TextKey::GraphNoOccurrenceEdges)));
    assert!(html.contains(t(Locale::En, TextKey::TableAndFilters)));
    assert!(!html.contains(CANVAS_ID));
}

#[test]
fn search_results_table_exposes_view_in_network_only_when_non_empty_and_never_loads_webgl_js() {
    let non_empty_html = render_dom(
        TableHarness,
        WorkspaceHarnessProps::simple(Arc::new(sample_result_set(2)), true),
    );
    assert!(non_empty_html.contains(t(Locale::En, TextKey::ViewInNetwork)));
    assert!(!non_empty_html.contains("lotus-graph-gl.js"));

    let empty_html = render_dom(
        TableHarness,
        WorkspaceHarnessProps::simple(Arc::new(ColumnarResultSet::default()), true),
    );
    assert!(!empty_html.contains(t(Locale::En, TextKey::ViewInNetwork)));
    assert!(!empty_html.contains("lotus-graph-gl.js"));
}

#[test]
fn default_max_nodes_constant_matches_spec() {
    assert_eq!(DEFAULT_MAX_NODES, 5_000);
}

#[test]
fn tripartite_projection_marks_all_three_legend_pills_active_without_two_type_note() {
    let set = Arc::new(sample_result_set(4));
    let rows: Vec<u32> = (0..4).collect();
    let model = Arc::new(build_graph(
        &set,
        &rows,
        GraphProjection::Tripartite,
        GraphScope::default(),
    ));
    let html = render_dom(
        StageAndInspectorHarness,
        SelectedInspectorHarnessProps {
            model,
            set,
            scope: GraphScope::default(),
            selected: None,
        },
    );
    assert_eq!(
        html.matches(r#"aria-pressed="true""#).count(),
        3,
        "all three entity toggles must be pressed in Tripartite mode:\n{html}"
    );
    assert!(
        !html.contains(r#"aria-pressed="false""#),
        "no entity toggle should be unpressed in Tripartite mode:\n{html}"
    );
    assert!(
        !html.contains("Showing 2 types"),
        "2-type note must not appear when all 3 types are shown:\n{html}"
    );
}

#[test]
fn copy_button_retains_distinct_title_and_aria_label() {
    #[component]
    fn CopyHarness() -> Element {
        rsx! {
            crate::components::copy_button::CopyButton {
                text: Arc::<str>::from("https://lotus.nprod.net/"),
                title: "Copy shareable link",
                locale: Locale::En,
            }
        }
    }

    let mut dom = VirtualDom::new(CopyHarness);
    dom.rebuild_in_place();
    let html = dioxus_ssr::render(&dom);
    assert!(html.contains(r#"title="Copy shareable link""#), "{html}");
    assert!(
        html.contains(r#"aria-label="Copy shareable link""#),
        "{html}"
    );
}

#[test]
fn network_workspace_renders_shareable_url_and_hydrates_all_viz_params() {
    let set = Arc::new(sample_result_set(4));
    let criteria = SearchCriteria {
        taxon: "Gentiana lutea".into(),
        ..SearchCriteria::up_to_year(2026)
    };
    let params = QueryParams::from([
        ("projection".to_owned(), "tripartite".to_owned()),
        ("mode".to_owned(), "2d".to_owned()),
        ("paused".to_owned(), "true".to_owned()),
        ("focus".to_owned(), "compound-Q100".to_owned()),
        ("max_neighbors".to_owned(), "120".to_owned()),
        ("selected".to_owned(), "taxon-Q2000".to_owned()),
    ]);
    let initial_view = Some(parse_graph_view_from_params(&params));

    let html = render_dom(
        WorkspaceHarness,
        WorkspaceHarnessProps {
            set,
            searched: true,
            criteria,
            initial_view,
        },
    );
    assert!(
        html.contains(
            "/network?execute=true&#38;focus=compound-Q100&#38;max_neighbors=120&#38;mode=2d&#38;paused=true&#38;projection=tripartite&#38;selected=taxon-Q2000&#38;taxon=Gentiana%20lutea"
        ),
        "ShareNotice must render the complete /network shareable URL:\n{html}"
    );
    assert!(
        html.contains(t(Locale::En, TextKey::GraphResumeMotion))
            && html.contains(r#"aria-pressed="true""#),
        "paused=true must hydrate Resume motion button state:\n{html}"
    );
    assert!(
        html.contains(t(Locale::En, TextKey::GraphEgoBadgePrefix)),
        "focus must render the ego badge:\n{html}"
    );
    assert!(
        html.contains(t(Locale::En, TextKey::CloseInspector))
            && html.contains("Q2000")
            && html.contains("Gentiana sp0"),
        "selected=taxon-Q2000 must hydrate SelectedNodeView in GraphInspector:\n{html}"
    );
    assert!(
        !html.contains("Showing 2 types"),
        "projection=tripartite must show all 3 entity types:\n{html}"
    );
}

#[test]
fn network_workspace_hydrates_overview_cap_and_selected_from_url_params() {
    let set = Arc::new(sample_result_set(5));
    let criteria = SearchCriteria {
        taxon: "Gentiana".into(),
        ..SearchCriteria::up_to_year(2026)
    };
    let params = QueryParams::from([
        ("max_nodes".to_owned(), "1000".to_owned()),
        ("selected".to_owned(), "compound-Q101".to_owned()),
    ]);
    let initial_view = Some(parse_graph_view_from_params(&params));

    let html = render_dom(
        WorkspaceHarness,
        WorkspaceHarnessProps {
            set,
            searched: true,
            criteria,
            initial_view,
        },
    );
    assert!(
        html.contains(
            "/network?execute=true&#38;max_nodes=1000&#38;selected=compound-Q101&#38;taxon=Gentiana"
        ),
        "ShareNotice must expose max_nodes and selected in /network URL:\n{html}"
    );
    assert!(
        html.contains(t(Locale::En, TextKey::CloseInspector))
            && html.contains("Q101")
            && html.contains("Compound-1"),
        "selected=compound-Q101 must render SelectedNodeView for Compound-1:\n{html}"
    );
}

#[test]
fn graph_renderer_recovers_frames_and_clears_drag_on_visibility_and_blur() {
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let graph_js =
        std::fs::read_to_string(app_dir.join("public/assets/js/graph/lotus-graph-gl.js"))
            .expect("read lotus-graph-gl.js");
    assert!(
        graph_js.contains(r#""visibilitychange""#)
            && graph_js.contains("resizeCanvas();")
            && graph_js.contains("scheduleFrame();"),
        "lotus-graph-gl.js must re-measure and reschedule frames when returning visible"
    );
    assert!(
        graph_js.contains(r#""blur""#) && graph_js.contains("state.pointer.down = false"),
        "lotus-graph-gl.js must clear stuck pointer drag state on hide and window blur"
    );
}

#[test]
fn browser_streaming_and_yield_paths_never_use_zero_delay_set_timeout() {
    let app_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_files = [
        app_dir.join("src/repositories/hybrid.rs"),
        app_dir.join("src/upload.rs"),
        app_dir.join("../../crates/lotus-search/src/reqwest_client/wasm.rs"),
    ];
    for path in repo_files {
        let source = std::fs::read_to_string(&path).expect("read browser async source");
        assert!(
            !source.contains("TimeoutFuture::new(0)"),
            "{} must yield via MessageChannel (`yield_to_browser`) instead of `TimeoutFuture::new(0)`",
            path.display()
        );
    }
}
