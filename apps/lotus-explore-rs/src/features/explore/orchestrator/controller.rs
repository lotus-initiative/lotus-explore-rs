// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::explore::command::SearchCommand;
use crate::services::search_telemetry as telemetry;
use dioxus::core::Task;
use lotus_search::SearchCriteria;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone, Default)]
pub struct SearchTaskController {
    in_flight: Rc<RefCell<Option<Task>>>,
    active: Rc<RefCell<Option<ActiveSearch>>>,
    next_run_id: Rc<Cell<u64>>,
}

#[derive(PartialEq)]
struct ActiveSearch {
    run_id: u64,
    criteria: SearchCriteria,
    command: SearchCommand,
}

impl SearchTaskController {
    pub fn new() -> Self {
        Self::default()
    }

    pub(super) fn try_begin(
        &self,
        criteria: &SearchCriteria,
        command: SearchCommand,
    ) -> Option<u64> {
        let mut active = self.active.borrow_mut();
        if active
            .as_ref()
            .is_some_and(|active| active.criteria == *criteria && active.command == command)
        {
            return None;
        }

        let run_id = self.next_run_id.get().wrapping_add(1);
        self.next_run_id.set(run_id);
        *active = Some(ActiveSearch {
            run_id,
            criteria: criteria.clone(),
            command,
        });
        Some(run_id)
    }

    pub(super) fn finish(&self, run_id: u64) {
        let mut active = self.active.borrow_mut();
        if active
            .as_ref()
            .is_some_and(|active| active.run_id == run_id)
        {
            *active = None;
            *self.in_flight.borrow_mut() = None;
        }
    }

    pub fn replace_in_flight(&self, next: Task) {
        if let Some(prev) = self.in_flight.borrow_mut().replace(next) {
            prev.cancel();
            telemetry::search_inflight_cancelled();
        }
    }
}

#[cfg(test)]
#[path = "controller/tests.rs"]
mod tests;
