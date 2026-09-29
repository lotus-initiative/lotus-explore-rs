// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Typed interaction boundary for the Explore feature.

use crate::features::explore::actions::ExploreAction;
use crate::features::explore::command::SearchCommand;
use crate::features::explore::orchestrator::{SearchTaskController, start_search};
use crate::features::explore::search_state::{ExploreState, dispatch_explore_action};
use crate::models::{SearchCriteria, SortColumn};
use crate::repositories::HybridRepository;
use crate::state::FormCriteriaContext;
use dioxus::prelude::*;

#[derive(Clone)]
pub struct ExploreInteractions {
    criteria: Signal<SearchCriteria>,
    form: FormCriteriaContext,
    explore: Signal<ExploreState>,
    task_controller: SearchTaskController,
    repo: HybridRepository,
}

impl ExploreInteractions {
    pub const fn new(
        criteria: Signal<SearchCriteria>,
        form: FormCriteriaContext,
        explore: Signal<ExploreState>,
        task_controller: SearchTaskController,
        repo: HybridRepository,
    ) -> Self {
        Self {
            criteria,
            form,
            explore,
            task_controller,
            repo,
        }
    }

    pub fn search(&self) {
        self.form.mark_searched();
        self.start(SearchCommand::Interactive);
    }

    pub fn preview(&self) {
        self.start(SearchCommand::Interactive);
    }

    pub fn retry(&self) {
        self.start(SearchCommand::Interactive);
    }

    pub fn dismiss_error(&self) {
        dispatch_explore_action(self.explore, ExploreAction::ErrorDismissed);
    }

    pub fn toggle_sort(&self, column: SortColumn) {
        dispatch_explore_action(self.explore, ExploreAction::SortToggled(column));
    }

    fn start(&self, command: SearchCommand) {
        start_search(
            self.criteria,
            command,
            self.explore,
            self.task_controller.clone(),
            self.repo,
        );
    }
}

pub fn use_explore_interactions() -> ExploreInteractions {
    use_context::<ExploreInteractions>()
}
