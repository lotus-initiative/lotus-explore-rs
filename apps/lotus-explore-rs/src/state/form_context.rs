// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Search-form context with dirty tracking and action-based updates.

use crate::features::explore::form_actions::{FormAction, apply_form_action_mut};
use crate::models::SearchCriteria;
use dioxus::prelude::*;

#[derive(Clone, Copy)]
pub struct FormCriteriaContext {
    pub criteria: Signal<SearchCriteria>,
    baseline: Signal<SearchCriteria>,
}

impl FormCriteriaContext {
    pub const fn new(criteria: Signal<SearchCriteria>, baseline: Signal<SearchCriteria>) -> Self {
        Self { criteria, baseline }
    }

    pub fn update(&self, action: FormAction) {
        let mut criteria = self.criteria;
        criteria.with_mut(|criteria| apply_form_action_mut(criteria, action));
    }

    pub fn is_dirty(&self) -> bool {
        *self.criteria.read() != *self.baseline.read()
    }

    pub fn mark_searched(&self) {
        let current = self.criteria.peek().clone();
        let mut baseline = self.baseline;
        *baseline.write() = current;
    }
}

pub fn use_form_criteria_context() -> FormCriteriaContext {
    use_context::<FormCriteriaContext>()
}
