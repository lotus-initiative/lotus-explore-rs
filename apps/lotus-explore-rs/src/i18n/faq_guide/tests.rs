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
                "{}: the {locale:?} summary is shorter than a sentence, so a collapsed                      section says nothing",
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
