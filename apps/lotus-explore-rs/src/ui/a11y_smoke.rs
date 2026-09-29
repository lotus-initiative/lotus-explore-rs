// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Lightweight accessibility smoke tests.

#[cfg(test)]
mod tests {
    #[test]
    fn main_landmark_is_labelled_and_skip_link_targets_it() {
        let shell_src = include_str!("../app/shell.rs");
        assert!(shell_src.contains("href: SKIP_TO_RESULTS_HREF"));
        assert!(shell_src.contains("id: MAIN_PANEL_ID"));
        assert!(shell_src.contains("aria_labelledby: PAGE_TITLE_ID"));
    }

    #[test]
    fn search_panel_exposes_heading_and_body_landmarks() {
        let search_panel_src = include_str!("../components/search_panel.rs");
        assert!(search_panel_src.contains("id: SEARCH_PANEL_BODY_ID"));
    }

    #[test]
    fn sortable_headers_expose_action_oriented_aria_label() {
        let header_src = include_str!("../components/results_table/table_header.rs");
        assert!(header_src.contains("aria_sort_toggle"));
        assert!(header_src.contains("aria_label: \"{sort_aria}\""));
    }

    #[test]
    fn results_expose_stable_domain_rdfa_contract() {
        let list_src = include_str!("../components/results_table.rs");
        let row_src = include_str!("../components/results_table/row_cells/render.rs");
        let compound_src = include_str!("../components/results_table/row_cells/cells/compound.rs");
        let taxon_src = include_str!("../components/results_table/row_cells/cells/taxon.rs");
        let reference_src =
            include_str!("../components/results_table/row_cells/cells/reference.rs");

        assert!(list_src.contains("\"vocab\": \"https://schema.org/\""));
        assert!(list_src.contains("\"typeof\": \"ItemList\""));
        assert!(row_src.contains("\"typeof\": \"ChemicalEntity\""));
        assert!(row_src.contains("\"data-lotus-id\": \"compound:{compound_qid}\""));
        assert!(compound_src.contains("\"property\": \"wdt:P235\""));
        assert!(taxon_src.contains("\"property\": \"wdt:P171\""));
        assert!(reference_src.contains("\"typeof\": \"ScholarlyArticle\""));
    }

    #[test]
    fn page_header_exposes_single_home_link_and_heading_id() {
        let header_src = include_str!("../components/layout/page_header.rs");
        assert!(header_src.contains("h1 { id: PAGE_TITLE_ID"));
        // The title link must carry a hover affordance that is not already its
        // resting state. `hover:no-underline` used to sit here, which measured as
        // no change at all, so the test pins the class that replaced it.
        assert!(
            header_src.contains("class: \"break-words text-text no-underline hover:text-accent\"")
        );
        // Home link uses visible text as accessible name (no redundant aria_label)
        assert!(header_src.contains("\"{t(locale, TextKey::PageTitle)}\""));
    }

    #[test]
    fn qs_dev_links_have_a_non_color_cue() {
        let curation_src = include_str!("../components/data_curation_page/sections/mod.rs");
        assert_eq!(
            curation_src
                .matches("class: \"font-medium text-accent underline\"")
                .count(),
            2
        );
    }

    #[test]
    fn landing_and_not_found_expose_headings_and_actions() {
        let landing_src = include_str!("../components/landing.rs");
        assert!(landing_src.contains("id: \"landing-welcome-heading\""));
        assert!(landing_src.contains("href_with_current_query(\"/search\")"));
        assert!(landing_src.contains("id: \"not-found-heading\""));
        assert!(landing_src.contains("href_with_current_query(\"/\")"));
    }

    #[test]
    fn stats_group_is_not_navigation() {
        let stat_bar_src =
            include_str!("../components/results_table/table_toolbar_sections/stat_bar.rs");
        assert!(stat_bar_src.contains("role: \"group\""));
        assert!(!stat_bar_src.contains("nav {"));
    }

    #[test]
    fn boot_theme_uses_shared_tokens() {
        let index_src = include_str!("../../index.html");
        let logo_src = include_str!("../../public/favicon.svg");

        assert!(index_src.contains("background: var(--shell-page-bg, #f2f5f8)"));
        assert!(index_src.contains("color: var(--text, #111827)"));
        // the lockup inherits the surrounding text colour (the outlined wordmark
        // carries fill="currentColor")
        assert!(logo_src.contains("currentColor"));
    }

    /// The logo must keep the official lotus lockup *and* render it without
    /// depending on a font being installed.
    #[test]
    fn logo_keeps_the_official_lockup_without_a_font_dependency() {
        let mark_src = include_str!("../../public/favicon.svg");
        let header_src = include_str!("../components/layout/page_header.rs");
        let styles_src = include_str!("../../tailwind/styles.css");

        assert!(
            !mark_src.contains("<text"),
            "favicon.svg must not contain a <text> element: a substituted font \
             changes the rendered extents per platform, which is what clipped \
             the wordmark on iOS"
        );
        assert!(
            header_src.contains("public/favicon.svg"),
            "the header must embed the official lockup, favicon.svg"
        );
        assert!(
            mark_src.contains("AlbertSans-Light") || mark_src.contains("wordmark: outlined"),
            "the wordmark must be present as outlined paths, not deleted"
        );

        // Lints here deny panic!/expect/indexing, so the invariant is carried
        // by asserts and plain iterator reads instead.
        let view_box = mark_src
            .split_once("viewBox=\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map_or("", |(value, _)| value);
        let mut parts = view_box
            .split_whitespace()
            .filter_map(|n| n.parse::<f64>().ok());
        let _min_x = parts.next();
        let _min_y = parts.next();
        let w = parts.next();
        let h = parts.next();
        let extra = parts.next();
        assert!(
            matches!(w, Some(width) if width > 0.0)
                && matches!(h, Some(height) if height > 0.0)
                && extra.is_none(),
            "favicon.svg viewBox must be \"minX minY width height\" with a positive size"
        );
        let (w, h) = (w.unwrap_or_default(), h.unwrap_or_default());

        let expected = format!("aspect-ratio: {w} / {h};");
        assert!(
            styles_src.contains(&expected),
            "brand-logo must pin aspect-ratio: {w} / {h}; to match the logo's \
             viewBox, otherwise the box can round onto the artwork edge and clip it"
        );
    }
}
