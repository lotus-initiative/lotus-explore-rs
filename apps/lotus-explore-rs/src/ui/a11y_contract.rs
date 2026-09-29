// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared accessibility IDs and landmark contracts.
//!
//! These are the values the app renders into `id`, `aria-labelledby` and
//! `href="#…"`. Keeping them in one place is what makes a rename a compile
//! error rather than a broken skip link; the invariants they must satisfy are
//! asserted in `a11y_smoke`.

pub const MAIN_PANEL_ID: &str = "main-panel";
pub const SKIP_TO_RESULTS_HREF: &str = "#main-panel";

pub const PAGE_TITLE_ID: &str = "page-title";

pub const SEARCH_PANEL_BODY_ID: &str = "search-panel-body";

pub const RESULTS_SECTION_ID: &str = "results-section";
pub const RESULTS_SECTION_HEADING_ID: &str = "results-section-heading";
