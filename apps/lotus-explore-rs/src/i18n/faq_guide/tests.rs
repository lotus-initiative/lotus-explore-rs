// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `faq_guide`, in their own file.

#![allow(clippy::expect_used, clippy::panic)]

use super::{FaqBlock, Labels, SECTIONS, untranslated_notice};
use crate::i18n::Locale;

const LOCALES: [Locale; 4] = [Locale::En, Locale::Fr, Locale::De, Locale::It];

#[test]
fn every_section_has_a_title_and_summary_in_every_locale() {
    for section in SECTIONS {
        for locale in LOCALES {
            assert!(
                !section.title(locale).trim().is_empty(),
                "{} has no title in {locale:?}",
                section.id
            );
            assert!(
                !section.summary(locale).trim().is_empty(),
                "{} has no summary in {locale:?}",
                section.id
            );
        }
    }
}

#[test]
fn the_summary_is_long_enough_to_be_a_summary() {
    // A collapsed section shows only its summary, so one shorter than its title
    // teaches nothing for opening it.
    for section in SECTIONS {
        for locale in LOCALES {
            assert!(
                section.summary(locale).chars().count() > 30,
                "{}: the {locale:?} summary is shorter than a sentence, so a collapsed section says nothing",
                section.id
            );
        }
    }
}

#[test]
fn no_locale_serves_the_english_title_or_summary() {
    // English fallback is invisible in review and obvious to a reader.
    for section in SECTIONS {
        for locale in [Locale::Fr, Locale::De, Locale::It] {
            assert_ne!(
                section.title(locale),
                section.title(Locale::En),
                "{} is untranslated for {locale:?}",
                section.id
            );
            assert_ne!(
                section.summary(locale),
                section.summary(Locale::En),
                "{} is untranslated for {locale:?}",
                section.id
            );
        }
    }
}

#[test]
fn section_anchors_are_unique_and_url_clean() {
    let mut seen: Vec<&str> = Vec::new();
    for section in SECTIONS {
        assert!(
            !seen.contains(&section.id),
            "duplicate anchor {}",
            section.id
        );
        assert!(
            section
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{} is not a clean URL fragment",
            section.id
        );
        seen.push(section.id);
    }
}

#[test]
fn no_section_is_empty() {
    // A disclosure that opens onto nothing is a dead end.
    for section in SECTIONS {
        assert!(!section.blocks.is_empty(), "{} has no content", section.id);
        // And it must not be *only* headings.
        assert!(
            section
                .blocks
                .iter()
                .any(|b| matches!(b, FaqBlock::Text(_) | FaqBlock::Table(_))),
            "{} has headings but no prose",
            section.id
        );
    }
}

#[test]
fn every_table_has_a_header_and_at_least_one_row() {
    // A one-row "table" is a header with no data, which reads as an empty section.
    for section in SECTIONS {
        for block in section.blocks {
            let FaqBlock::Table(rows) = block else {
                continue;
            };
            assert!(
                rows.len() >= 2,
                "{}: a table needs a header and a row",
                section.id
            );
            let width = rows.first().map_or(0, |header| header.len());
            assert!(
                width > 1,
                "{}: a single-column table is a paragraph",
                section.id
            );
            for (index, row) in rows.iter().enumerate() {
                assert_eq!(
                    row.len(),
                    width,
                    "{}: row {index} has {} cells against a {width}-cell header",
                    section.id,
                    row.len()
                );
            }
        }
    }
}

#[test]
fn the_notice_appears_for_every_locale_but_english() {
    // The body is English, so a reader in another language is told at the point they
    // will see it rather than inferring it.
    assert!(
        untranslated_notice(Locale::En).is_none(),
        "an English reader does not need to be told the page is in English"
    );
    for locale in [Locale::Fr, Locale::De, Locale::It] {
        let Some((label, detail)) = untranslated_notice(locale) else {
            panic!("{locale:?} has no notice for the English-only body");
        };
        assert!(!label.trim().is_empty(), "{locale:?} notice label is empty");
        assert!(
            detail.chars().count() > 40,
            "{locale:?} notice says nothing"
        );
    }
}

#[test]
fn labels_pick_the_locale_that_was_asked_for() {
    // The four arms are positional, so a mistake serves one language to every reader.
    let labels = Labels {
        en: "english",
        fr: "french",
        de: "german",
        it: "italian",
    };
    assert_eq!(labels.pick(Locale::En), "english");
    assert_eq!(labels.pick(Locale::Fr), "french");
    assert_eq!(labels.pick(Locale::De), "german");
    assert_eq!(labels.pick(Locale::It), "italian");
}

/// Tokens that are names rather than quantities, removed before the scan: a
/// Wikidata item or property id, an RFC number, a version number, and the two
/// example chemistry strings. Everything else the guide says has to be expressed
/// in words, because a bare number here has nothing to notice it going stale.
const IDENTIFIERS: [&str; 15] = [
    // Wikidata items and properties.
    "Q128267",
    "Q23118",
    "P1420",
    "P566",
    "P1403",
    "P694",
    "P1843",
    "P225",
    "P235",
    "P1531",
    // The two example chemistry strings.
    "DBOVHQOUSDWAPQ-WTONXPSSSA-N",
    "C[C@H](O)CO",
    // Standards and browser versions: names, not quantities.
    "4180",
    "15.2",
    "111",
];

/// Every string the guide renders, in every locale.
fn guide_strings() -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    for section in SECTIONS {
        for locale in LOCALES {
            out.push((section.id, section.title(locale)));
            out.push((section.id, section.summary(locale)));
        }
        for block in section.blocks {
            match block {
                FaqBlock::Heading(t) | FaqBlock::Text(t) | FaqBlock::Note(t) => {
                    out.push((section.id, t));
                }
                FaqBlock::Table(rows) => {
                    for row in *rows {
                        out.extend(row.iter().map(|cell| (section.id, *cell)));
                    }
                }
                FaqBlock::List(items) => {
                    for item in *items {
                        out.push((section.id, item));
                    }
                }
            }
        }
    }
    out
}

#[test]
fn no_bare_number_in_the_guide_can_go_stale() {
    // The gate the FAQ numbers need. A count or a timing held as prose has
    // nothing to notice it going out of date: three here were wrong against the
    // code before this existed -- a row ceiling, a language-tag count and a
    // candidate count -- and no reader could tell.
    for (section, text) in guide_strings() {
        let mut stripped = text.to_string();
        for identifier in IDENTIFIERS {
            stripped = stripped.replace(identifier, "");
        }
        if let Some(at) = stripped.find(|c: char| c.is_ascii_digit()) {
            let token: String = stripped[at..]
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == ',' || *c == '.')
                .collect();
            panic!("{section}: the guide still carries `{token}`, in {text:?}");
        }
    }
}

#[test]
fn the_allowlist_held_nothing_back_that_was_removed() {
    // The list above is a hole in the gate, so a stale entry in it would let a
    // number back in silently. Each token must still occur in the guide.
    let cells: Vec<&str> = guide_strings().into_iter().map(|(_, text)| text).collect();
    for identifier in IDENTIFIERS {
        assert!(
            cells.iter().any(|cell| cell.contains(identifier)),
            "{identifier} is allowlisted but no longer appears in the guide"
        );
    }
}
