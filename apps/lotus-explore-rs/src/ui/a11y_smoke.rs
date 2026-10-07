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

    /// The files the app pins the icon to, in place of the OS-following one.
    ///
    /// `favicon.svg` answers for the reader's OS. These exist for the states
    /// the OS cannot describe: the theme toggle overriding it, and
    /// `?dark_mode=` on a shared link.
    const PINNED_LIGHT: &str = include_str!("../../public/favicon-light.svg");
    const PINNED_DARK: &str = include_str!("../../public/favicon-dark.svg");

    fn pinned() -> [(&'static str, &'static str); 2] {
        [
            ("favicon-light.svg", PINNED_LIGHT),
            ("favicon-dark.svg", PINNED_DARK),
        ]
    }

    /// The only element whose colour follows the theme.
    ///
    /// The flower is deliberately not in here. Reported: the ask was for the
    /// icon's *text* to switch, and recolouring the petals made the mark read
    /// as two different logos depending on the theme. The wordmark is five
    /// `<path>` elements in one `<g>`; the flower is everything else.
    const THEMED: &str = "lotus-wordmark";

    /// The mark with the stylesheet and the palette hexes blanked out: the two
    /// things the three files are meant to disagree about, and nothing else.
    fn skeleton(svg: &str) -> String {
        let head = svg
            .split_once("<style type=\"text/css\">")
            .map_or("", |(head, _)| head);
        let tail = svg.rsplit_once("</style>").map_or("", |(_, tail)| tail);
        let mut body = format!("{head}@@{tail}");
        {
            let start = format!("class=\"{THEMED}\" fill=\"");
            let mut from = 0;
            // Every occurrence, not the first: `lotus-ink` is three separate
            // paths, and blanking only the first would leave the other two
            // comparing their real hexes.
            while let Some(at) = body[from..].find(&start) {
                let after = from + at + start.len();
                let end = body[after..].find('"').map_or(body.len(), |n| after + n);
                body.replace_range(after..end, "@@");
                from = after + 2;
            }
        }
        body
    }

    #[test]
    fn the_pinned_icons_are_the_same_artwork() {
        // Three files hold one drawing, because a favicon cannot pull its
        // artwork from another file. Nothing stops a petal being added to just
        // one of them -- and the wrong one is invisible, because the icon still
        // looks like a logo, just not like the other theme's logo. So compare
        // what is left once the palette and the stylesheet are removed: if the
        // artwork changes, this fails and the other two must be regenerated.
        let canonical = skeleton(MARK);
        for (file, svg) in pinned() {
            assert_eq!(
                skeleton(svg),
                canonical,
                "{file} is not the same drawing as favicon.svg"
            );
        }
    }

    #[test]
    fn a_pinned_icon_does_not_ask_the_os_anything() {
        // The pinned files exist precisely because the OS answer is the wrong
        // one. A `prefers-color-scheme` block left in one of them would still
        // be consulted, and on the machine that needs the pin -- the reader who
        // overrode their OS -- it would hand back the colour they chose
        // against, which is the bug this pair was added to close.
        for (file, svg) in pinned() {
            // No `@media` at all rather than no mention of the feature: the
            // comment in each file explains the rule by name, and the defect is
            // a live media query, not the word.
            assert!(
                !svg.contains("@media"),
                "{file} is pinned for a theme the app already decided, but still \
                 carries a media query, so the icon can disagree with the page"
            );
        }
    }

    #[test]
    fn the_pinned_icons_paint_the_wordmark() {
        // With no stylesheet block to fall back on, the fill has to be on the
        // element. A class that lost its `fill` renders as flat black, which
        // passes every geometry check above.
        for (file, svg) in pinned() {
            assert!(
                svg.contains(&format!("class=\"{THEMED}\" fill=\"#")),
                "{file} has no fill on the wordmark, so it renders black"
            );
        }
    }

    #[test]
    fn only_the_wordmark_follows_the_theme() {
        // The regression guard for the over-reach: the dark palette started out
        // recolouring the petals too, which was not what was asked for and
        // split one logo into two. The flower must keep its brand colours in
        // every file, and the only selector the dark block may name is the
        // wordmark.
        const BRAND_FILLS: [&str; 4] = ["#484848", "#900", "#069", "#396"];
        for (file, svg) in std::iter::once(("favicon.svg", MARK)).chain(pinned()) {
            for colour in BRAND_FILLS {
                assert!(
                    svg.contains(&format!("style=\"fill:{colour};fill-opacity:1")),
                    "{file} dropped the flower's {colour} petal, so the logo is \
                     no longer the logo"
                );
            }
        }
        // Bounded by the style block: everything after the media query would
        // otherwise run on into the artwork, where the wordmark element is.
        let dark_block = MARK
            .split_once("@media (prefers-color-scheme: dark)")
            .and_then(|(_, tail)| tail.split_once("</style>").map(|(block, _)| block))
            .unwrap_or_default();
        // Every class name the dark block mentions. Splitting on punctuation
        // says what it means without walking the nested braces: the wordmark is
        // the only element allowed to be named in there.
        let named: Vec<&str> = dark_block
            .split(|c: char| !(c.is_alphanumeric() || c == '-'))
            .filter(|token| token.starts_with("lotus-"))
            .collect();
        assert_eq!(
            named,
            [THEMED],
            "the dark palette also themes {named:?}, but only the wordmark may \
             follow the theme"
        );
    }

    #[test]
    fn the_wordmark_is_not_painted_inline() {
        // An inline `style="fill:…"` attribute beats every stylesheet rule that
        // is not `!important`, so a wordmark carrying one would ignore the
        // dark palette entirely -- and the file would still look right in a
        // light tab, which is why this is invisible until someone switches to
        // dark.
        //
        // Scoped to the wordmark on purpose. The flower paints itself inline and
        // always did; it no longer follows the theme, so there is nothing to
        // override. An earlier version of this walked the whole file and passed
        // for the wrong reason: it stripped comments by keeping only the text
        // before the first `<!--`, which threw away every live element after
        // it -- the flower, and the bug it was looking for.
        let wordmark = MARK
            .split_once(&format!("class=\"{THEMED}\""))
            .and_then(|(_, rest)| rest.split_once('>').map(|(_, after)| after))
            .map_or("", |after| after.split("</g>").next().unwrap_or_default());
        assert!(
            !wordmark.is_empty(),
            "the {THEMED} group was not found, so this asserts nothing"
        );
        assert!(
            !wordmark.contains("style=\"fill:"),
            "the wordmark paints itself inline, so the dark palette cannot reach \
             it: {wordmark}"
        );
        assert!(
            MARK.contains(&format!("class=\"{THEMED}\" fill=\"#")),
            "the wordmark needs a fill on the element, or it renders black in \
             the browsers that ignore the media query"
        );
    }

    #[test]
    fn the_wordmark_does_not_lean_on_currentcolor() {
        // `currentColor` in a favicon resolves against the SVG's own document,
        // which inherits nothing -- so it renders as the default black and can
        // never follow the page's theme. That is what the wordmark was painted
        // with.
        assert!(
            !MARK.contains("currentColor"),
            "favicon.svg paints with currentColor, which cannot see the page's \
             theme: the tab icon is black whatever the reader chose"
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
