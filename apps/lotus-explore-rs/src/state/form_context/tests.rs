// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The two questions the reset button's enabled state depends on, and the
//! difference between them, which is the whole reason there are two methods.
//!
//! Every test runs inside a `VirtualDom`'s runtime scope, because a `Signal` is
//! owned by the runtime that made it: constructing one outside panics with
//! "Must be called from inside a Dioxus runtime", which is a fact about Dioxus
//! rather than about this code and so belongs here rather than in each test.

#![allow(clippy::expect_used)]

use super::*;
use crate::clock::current_year;

/// Run `body` with a Dioxus runtime **and a scope** installed.
///
/// Both, because `Signal::new` needs an owner and an owner comes from a scope:
/// a runtime alone is what `VirtualDom::in_runtime` gives, and it fails with
/// `Option::unwrap()` on `None` one line further in. The tree is built first,
/// which is what creates the root scope.
fn in_runtime(body: impl FnOnce()) {
    let mut dom = VirtualDom::new(|| rsx! {});
    dom.rebuild_in_place();
    dom.in_scope(ScopeId::ROOT, body);
}

/// A context whose criteria and baseline both start pristine.
fn context() -> FormCriteriaContext {
    let pristine = SearchCriteria::up_to_year(current_year());
    FormCriteriaContext::new(Signal::new(pristine.clone()), Signal::new(pristine))
}

#[test]
fn a_pristine_form_has_no_filters_to_reset() {
    in_runtime(|| {
        let ctx = context();
        assert!(!ctx.has_filters(current_year()));
        assert!(!ctx.is_dirty());
    });
}

#[test]
fn a_form_loaded_from_a_link_has_filters_even_though_it_is_clean() {
    // The distinction the two predicates exist for. A share link fills the
    // form at startup, so the baseline is seeded from it and the form is not
    // dirty -- but four fields are set, and a reset button disabled by
    // `is_dirty` would leave that visitor no way back.
    in_runtime(|| {
        let year = current_year();
        // Built the way `bootstrap.rs` builds it for a share link: the form
        // arrives already filled, and the baseline is seeded from it.
        let mut loaded = SearchCriteria::up_to_year(year);
        loaded.taxon = "Gentiana lutea".into();
        let ctx = FormCriteriaContext::new(Signal::new(loaded.clone()), Signal::new(loaded));

        assert!(
            !ctx.is_dirty(),
            "nothing has been searched since it arrived"
        );
        assert!(ctx.has_filters(year), "but there is a filter to clear");
    });
}

#[test]
fn a_reset_clears_every_field_and_leaves_the_form_dirty() {
    in_runtime(|| {
        let year = current_year();
        let ctx = context();
        ctx.update(FormAction::Taxon("Gentiana lutea".into()));
        ctx.update(FormAction::MassMin(120.0));
        ctx.update(FormAction::FormulaEnabled(true));
        ctx.mark_searched();
        assert!(!ctx.is_dirty(), "the search caught up with the form");

        ctx.reset(year);

        assert!(!ctx.has_filters(year), "nothing is set any more");
        assert!(
            ctx.is_dirty(),
            "and the results still on screen are not the empty search, so \
             the form is dirty against them"
        );
        assert_eq!(*ctx.criteria.read(), SearchCriteria::up_to_year(year));
    });
}

#[test]
fn a_reset_keeps_the_year_the_criteria_are_read_against() {
    // The year is an argument rather than something read here, because it is
    // part of what is being restored: a reset that recomputed it would
    // quietly redefine a field nobody touched.
    in_runtime(|| {
        let year = current_year();
        let ctx = context();
        ctx.update(FormAction::Taxon("Gentiana lutea".into()));

        ctx.reset(year - 5);

        assert_eq!(
            ctx.criteria.read().year_max,
            year - 5,
            "the year the reset was given is the year in the form"
        );
        assert!(
            ctx.has_filters(year),
            "and against the current year it is not the default, which is \
             what a form carrying a different year_max should report"
        );
    });
}
