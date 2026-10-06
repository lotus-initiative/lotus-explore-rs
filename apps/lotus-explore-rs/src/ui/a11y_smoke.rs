// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The accessibility properties the app claims, checked where they are defined.
//!
//! This file used to read its own source and assert that the strings it found
//! were the strings it expected -- `shell_src.contains("id: MAIN_PANEL_ID")`.
//! That passes if the markup is right and also if a `sed` left the name
//! behind in a comment, and it fails when rustfmt wraps a line, which is a
//! change to a test with no change to the app.
//!
//! The checks below are the same facts, sourced from `a11y_contract` rather
//! than from the text of a file. A constant that is defined but never applied
//! would slip past both; that is caught by the render tests, which assert on
//! the virtual DOM.

#[cfg(test)]
#[path = "a11y_smoke/tests.rs"]
mod tests;

/// The brand assets, checked against the files rather than against source text.
///
/// The wordmark was once a `<text>` element. A substituted font changes the
/// rendered extents per platform, and the box was sized from a guess, so the
/// wordmark clipped on iOS. Both halves of the fix are load-bearing: outlined
/// paths, and an `aspect-ratio` taken from the artwork's own `viewBox`. A test
/// that reads the file can check the second without a browser.
mod brand {
    /// The lockup the header renders: the flower above the wordmark.
    const MARK: &str = include_str!("../../public/favicon.svg");
    const STYLES: &str = include_str!("../../tailwind/styles.css");

    /// The `width` and `height` from the viewBox, which is what the box must
    /// match or the artwork is cropped.
    fn artwork_extent() -> (f64, f64) {
        let view_box = MARK
            .split_once("viewBox=\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map_or("", |(value, _)| value);
        let mut parts = view_box
            .split_whitespace()
            .filter_map(|n| n.parse::<f64>().ok());
        // minX, minY, width, height -- the two minima are not needed.
        let (_min_x, _min_y, width, height) =
            (parts.next(), parts.next(), parts.next(), parts.next());
        assert!(parts.next().is_none(), "a viewBox has exactly four numbers");
        (width.unwrap_or_default(), height.unwrap_or_default())
    }

    #[test]
    fn the_lockup_does_not_depend_on_an_installed_font() {
        assert!(
            !MARK.contains("<text"),
            "favicon.svg contains a <text> element: a substituted font changes \
             the rendered extents per platform, which is what clipped the \
             wordmark on iOS"
        );
    }

    #[test]
    fn the_lockup_keeps_its_gradients() {
        // The petals are painted with `url(#…)` references. Dropping the
        // `<defs>` block while extracting the mark leaves valid SVG that
        // renders as flat black, which looks like a different logo rather than
        // a broken one.
        assert!(MARK.contains("<defs>"), "the mark lost its gradient defs");
        for id in ["a", "b", "c"] {
            let referenced = MARK.contains(&format!("url(#{id})"));
            assert_eq!(
                referenced,
                MARK.contains(&format!("id=\"{id}\"")),
                "gradient '{id}' is referenced and defined, or neither"
            );
        }
    }

    #[test]
    fn a_group_inside_a_landmark_does_not_repeat_its_name() {
        // Two nested elements with the same accessible name are announced
        // twice, so the section switcher was listed under "Choose a section"
        // and then again under "Choose a section".
        const SWITCH: &str = include_str!("../components/layout/view_switch.rs");
        let nav_label = SWITCH
            .split("aria_label:")
            .nth(1)
            .and_then(|rest| rest.split('"').nth(1))
            .unwrap_or_default();
        assert!(!nav_label.is_empty(), "the nav must have a name of its own");

        let group = SWITCH
            .split_once("SegmentedControl {")
            .map(|(_, rest)| rest.chars().take(200).collect::<String>())
            .unwrap_or_default();
        assert!(
            group.contains("aria_label: \"\""),
            "the group inside the nav must not repeat the nav's name"
        );
    }

    #[test]
    fn the_lockup_box_matches_the_artwork() {
        let (width, height) = artwork_extent();
        assert!(
            width > 0.0 && height > 0.0,
            "the viewBox has no positive size, so there is nothing to render"
        );
        // The numbers are rendered the way the stylesheet writes them, without
        // trailing zeros, so the test compares values rather than formatting.
        let render = |n: f64| {
            let s = format!("{n:.3}");
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        };
        let expected = format!("aspect-ratio: {} / {};", render(width), render(height));
        assert!(
            STYLES.contains(&expected),
            "the logo box must pin '{expected}' to the viewBox, or the box can \
             round onto the artwork edge and clip it"
        );
    }
}
