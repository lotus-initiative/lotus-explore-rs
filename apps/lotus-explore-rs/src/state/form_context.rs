// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Search-form context with dirty tracking and action-based updates.

use crate::features::explore::form_actions::{FormAction, apply_form_action_mut};
use dioxus::prelude::*;
use lotus_search::SearchCriteria;

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

    /// Every filter back to its default, for the current year.
    ///
    /// A method rather than a [`FormAction`] variant because it replaces the
    /// whole criteria rather than one field, and a variant would have to be a
    /// match arm that ignores its subject and assigns a fresh value -- which is a
    /// `FormAction` that lies about being an action.
    ///
    /// `year_max` rather than reading the clock here: this is state, and the year
    /// is a property of the criteria it is restoring. A reset that recomputed it
    /// would quietly change the meaning of a field nobody had touched.
    ///
    /// Note what this does *not* do: it does not [`mark_searched`](Self::mark_searched).
    /// The results on screen were produced by the criteria being discarded, so
    /// the form is now dirty against them and the viewport says so. Claiming the
    /// results match an empty search would be a lie in the only direction that
    /// matters.
    pub fn reset(&self, year_max: u16) {
        // The local binding is what `write` needs: it takes `&mut self`, and a
        // `Signal` handle is `Copy`, so this is the same shape `update` uses and
        // does not lock anything.
        let mut criteria = self.criteria;
        *criteria.write() = SearchCriteria::up_to_year(year_max);
    }

    /// Whether anything is set that a reset would clear.
    ///
    /// Not [`is_dirty`](Self::is_dirty): that compares the form against the last
    /// search, so a form loaded from a share link is clean while holding four
    /// filters -- and the reset button is exactly what that visitor needs. This
    /// compares against the defaults instead, which is the question the button's
    /// enabled state is asking.
    pub fn has_filters(&self, year_max: u16) -> bool {
        *self.criteria.peek() != SearchCriteria::up_to_year(year_max)
    }
}

pub fn use_form_criteria_context() -> FormCriteriaContext {
    use_context::<FormCriteriaContext>()
}
#[cfg(test)]
#[path = "form_context/tests.rs"]
mod tests;
