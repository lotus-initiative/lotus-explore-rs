// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared accessibility IDs and landmark contracts.

pub const MAIN_PANEL_ID: &str = "main-panel";
pub const SKIP_TO_RESULTS_HREF: &str = "#main-panel";

pub const PAGE_TITLE_ID: &str = "page-title";

pub const SEARCH_PANEL_BODY_ID: &str = "search-panel-body";

pub const RESULTS_SECTION_ID: &str = "results-section";
pub const RESULTS_SECTION_HEADING_ID: &str = "results-section-heading";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique_and_non_empty() {
        let ids = [
            MAIN_PANEL_ID,
            PAGE_TITLE_ID,
            SEARCH_PANEL_BODY_ID,
            RESULTS_SECTION_ID,
            RESULTS_SECTION_HEADING_ID,
        ];

        assert!(ids.iter().all(|id| !id.trim().is_empty()));

        let unique: HashSet<&str> = ids.into_iter().collect();
        assert_eq!(unique.len(), ids.len());
    }

    #[test]
    fn skip_link_points_to_main_panel() {
        assert!(SKIP_TO_RESULTS_HREF.starts_with('#'));
        assert_eq!(SKIP_TO_RESULTS_HREF, "#main-panel");
        assert_eq!(SKIP_TO_RESULTS_HREF.trim_start_matches('#'), MAIN_PANEL_ID);
    }
}
