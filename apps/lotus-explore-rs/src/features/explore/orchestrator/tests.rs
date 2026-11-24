// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

#![allow(clippy::manual_string_new)]
#![allow(clippy::panic)]

use super::controller::SearchTaskController;
use super::runtime::test_exports::{
    build_search_succeeded_action_for_tests, make_run_finish_guard_for_tests,
    validate_search_criteria_for_tests,
};
use super::start_search;
use dioxus::prelude::*;
use lotus_explore_core::explore::actions::ExploreAction;
use lotus_explore_core::explore::command::SearchCommand;
use lotus_explore_core::explore::outcome::SearchOutcome;
use lotus_explore_core::explore::request::SearchRequest;
use lotus_explore_core::explore::search_state::ExploreState;
use lotus_explore_core::explore::types::{DomainError, ValidationFault};
use lotus_model::SearchCriteria;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[test]
fn build_search_succeeded_action_applies_finalized_counts() {
    let request = SearchRequest::new(
        SearchCriteria::up_to_year(crate::clock::current_year()),
        SearchCommand::Interactive,
    );
    let outcome = SearchOutcome {
        set: std::sync::Arc::new(lotus_model::ColumnarResultSet::default()),
        qid: Some("Q42".to_string()),
        warnings: Vec::new(),
        query: "SELECT * WHERE {}".to_string(),
        display_capped_rows: true,
        endpoint: lotus_explore_core::export::metadata::SparqlEndpoint::Qlever,
    };

    let action = build_search_succeeded_action_for_tests(&request, outcome);
    match action {
        ExploreAction::SearchSucceeded {
            set,
            display_capped_rows,
            ..
        } => {
            // The counts are no longer carried on the action: the set holds them,
            // so there is no field that could disagree with the rows on screen.
            assert_eq!(set.row_count(), 0);
            assert_eq!(set.stats().n_entries, 0);
            assert!(display_capped_rows);
        }
        _ => panic!("expected SearchSucceeded action"),
    }
}

#[test]
fn validate_search_criteria_accepts_a_search_that_names_nothing() {
    // The orchestrator no longer refuses this. A search with no structure and no
    // taxon is answered, with a notice saying it covers all of LOTUS -- which is
    // what makes the answer legible rather than surprising.
    let criteria = SearchCriteria {
        taxon: " ".into(),
        structure: "".into(),
        formula_enabled: false,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };

    let result = validate_search_criteria_for_tests(&criteria);
    assert_eq!(result, Ok(()));
}

#[test]
fn validate_search_criteria_accepts_formula_only_input() {
    let criteria = SearchCriteria {
        taxon: "".into(),
        structure: "".into(),
        formula_enabled: true,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };

    assert_eq!(validate_search_criteria_for_tests(&criteria), Ok(()));
}

#[test]
fn validate_search_criteria_maps_shared_mass_validation_fault() {
    let criteria = SearchCriteria {
        taxon: "Rosa".into(),
        mass_min: -1.0,
        ..SearchCriteria::up_to_year(crate::clock::current_year())
    };

    assert_eq!(
        validate_search_criteria_for_tests(&criteria),
        Err(DomainError::Validation(ValidationFault::MassOutOfRange)),
        "negative mass must map to MassOutOfRange"
    );
}

type PendingBodyResult = Result<lotus_search::ResponseBody, crate::repositories::RepositoryError>;

#[derive(Clone)]
struct PendingRepo {
    rx: Arc<Mutex<Option<futures::channel::oneshot::Receiver<PendingBodyResult>>>>,
}

impl PartialEq for PendingRepo {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.rx, &other.rx)
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "LotusRepository is an AFIT: the `async fn` spelling is fixed by the trait"
)]
impl crate::repositories::LotusRepository for PendingRepo {
    async fn api_search(
        &self,
        _criteria: &SearchCriteria,
        _limit: usize,
        _include_counts: bool,
    ) -> Option<Result<crate::api::SearchResponse, crate::repositories::RepositoryError>> {
        None
    }

    async fn sparql_body(
        &self,
        _query: &str,
    ) -> Result<lotus_search::ResponseBody, crate::repositories::RepositoryError> {
        let rx = self.rx.lock().ok().and_then(|mut guard| guard.take());
        let Some(rx) = rx else {
            return Err(crate::repositories::RepositoryError::network(
                "pending test receiver already taken",
            ));
        };
        rx.await.unwrap_or_else(|_| {
            Err(crate::repositories::RepositoryError::network(
                "pending test sender dropped",
            ))
        })
    }
}

type CapturedHarness = (
    Signal<ExploreState>,
    Signal<SearchCriteria>,
    Signal<bool>,
    SearchTaskController,
);

#[derive(Clone)]
struct UnmountHarnessProps {
    repo: PendingRepo,
    captured: Rc<RefCell<Option<CapturedHarness>>>,
}

impl PartialEq for UnmountHarnessProps {
    fn eq(&self, other: &Self) -> bool {
        self.repo == other.repo && Rc::ptr_eq(&self.captured, &other.captured)
    }
}

#[component]
fn ChildSearchTrigger(
    criteria: Signal<SearchCriteria>,
    explore: Signal<ExploreState>,
    repo: PendingRepo,
) -> Element {
    let controller = use_context::<SearchTaskController>();
    use_hook(move || {
        start_search(
            criteria,
            SearchCommand::Interactive,
            explore,
            controller,
            repo,
        );
    });
    rsx! { div { "child" } }
}

#[component]
fn UnmountSearchHarness(props: UnmountHarnessProps) -> Element {
    let explore = use_signal(ExploreState::default);
    let criteria = use_signal(|| {
        let mut c = SearchCriteria::up_to_year(crate::clock::current_year());
        c.taxon = "Q158572".to_string();
        c
    });
    let show_child = use_signal(|| true);
    let controller = use_context_provider(SearchTaskController::new);

    let captured = props.captured.clone();
    use_hook(move || {
        *captured.borrow_mut() = Some((explore, criteria, show_child, controller));
    });

    rsx! {
        div {
            if *show_child.read() {
                ChildSearchTrigger {
                    criteria,
                    explore,
                    repo: props.repo,
                }
            }
        }
    }
}

#[test]
fn child_unmount_while_loading_does_not_cancel_search_or_strand_loading_state() {
    let (tx, rx) = futures::channel::oneshot::channel();
    let repo = PendingRepo {
        rx: Arc::new(Mutex::new(Some(rx))),
    };
    let captured = Rc::new(RefCell::new(None));
    let mut dom = VirtualDom::new_with_props(
        UnmountSearchHarness,
        UnmountHarnessProps {
            repo,
            captured: captured.clone(),
        },
    );
    dom.rebuild_in_place();
    for _ in 0..4 {
        dom.process_events();
        let _ = dom.render_immediate_to_vec();
    }

    let handles = captured
        .borrow()
        .clone()
        .unwrap_or_else(|| panic!("harness must capture root state"));
    let (explore, criteria, mut show_child, controller) = handles;

    dom.in_scope(ScopeId::ROOT, || {
        assert!(
            explore.read().lifecycle.loading,
            "search must be loading before child unmounts"
        );
        assert!(
            controller
                .try_begin(&criteria.read(), SearchCommand::Interactive)
                .is_none(),
            "duplicate search must be suppressed while loading"
        );
        show_child.set(false);
    });

    for _ in 0..4 {
        dom.process_events();
        let _ = dom.render_immediate_to_vec();
    }

    assert!(
        tx.send(Ok(lotus_search::ResponseBody::from_static(
            b"compound,taxon\nQ1,Q2\n"
        )))
        .is_ok(),
        "child unmount must not drop the root-scoped search receiver"
    );

    for _ in 0..8 {
        dom.process_events();
        let _ = dom.render_immediate_to_vec();
    }

    dom.in_scope(ScopeId::ROOT, || {
        let state = explore.read();
        assert!(
            !state.lifecycle.loading,
            "search must finish and clear loading even after triggering child unmounted"
        );
        assert!(
            state.lifecycle.searched_once,
            "search must commit searched_once after child unmounted"
        );
        assert!(
            state.lifecycle.error.is_none(),
            "search must complete without error after child unmounted"
        );
        assert_eq!(
            state.result.set.row_count(),
            1,
            "completed search must populate the parsed result row after child unmount"
        );
        assert!(
            controller
                .try_begin(&criteria.read(), SearchCommand::Interactive)
                .is_some(),
            "controller must clear active run when root-scoped task finishes"
        );
    });
}

#[test]
fn run_finish_guard_clears_matching_run_on_drop_and_ignores_stale_run() {
    let controller = SearchTaskController::new();
    let criteria = SearchCriteria::up_to_year(crate::clock::current_year());

    let run_1 = controller
        .try_begin(&criteria, SearchCommand::Interactive)
        .unwrap_or_else(|| panic!("first run must begin"));
    assert!(
        controller
            .try_begin(&criteria, SearchCommand::Interactive)
            .is_none(),
        "identical run must be suppressed while active"
    );

    let guard_1 = make_run_finish_guard_for_tests(controller.clone(), run_1);
    drop(guard_1);

    let run_2 = controller
        .try_begin(&criteria, SearchCommand::Interactive)
        .unwrap_or_else(|| panic!("dropping RunFinishGuard must free the active slot"));

    let guard_stale = make_run_finish_guard_for_tests(controller.clone(), run_1);
    drop(guard_stale);
    assert!(
        controller
            .try_begin(&criteria, SearchCommand::Interactive)
            .is_none(),
        "dropping a stale RunFinishGuard must not clear a newer active run"
    );

    let guard_2 = make_run_finish_guard_for_tests(controller.clone(), run_2);
    drop(guard_2);
    assert!(
        controller
            .try_begin(&criteria, SearchCommand::Interactive)
            .is_some(),
        "dropping the current RunFinishGuard must release the slot"
    );
}
