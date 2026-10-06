// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `faq`, in their own file.

use super::{ENTRIES, FaqCategory, FaqEntry};
use crate::i18n::Locale;

const LOCALES: [Locale; 4] = [Locale::En, Locale::Fr, Locale::De, Locale::It];

/// A string field of a JSON value, or `None` if absent or not a string.
///
/// A function rather than `value["key"]` because the workspace denies
/// `indexing_slicing`: an index returning `Null` fails the assertion noisily.
fn text<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

#[test]
fn every_question_has_an_answer_in_every_locale() {
    for entry in ENTRIES {
        for locale in LOCALES {
            assert!(
                !entry.question(locale).is_empty(),
                "{} has no question in {locale:?}",
                entry.id
            );
            assert!(
                !entry.answer(locale).is_empty(),
                "{} has no answer in {locale:?}",
                entry.id
            );
        }
    }
}

#[test]
fn every_locale_gives_a_distinct_translation() {
    // A locale that silently fell back to English would look fine in the app and
    // be wrong, which is the failure this catches: four rows of English presented
    // as four locales.
    for entry in ENTRIES {
        let english = entry.question(Locale::En);
        for locale in [Locale::Fr, Locale::De, Locale::It] {
            assert_ne!(
                entry.question(locale),
                english,
                "{} is untranslated for {locale:?}",
                entry.id
            );
        }
    }
}

#[test]
fn question_anchors_are_unique_and_usable_in_a_url() {
    let mut seen: Vec<&str> = Vec::new();
    for entry in ENTRIES {
        assert!(
            !seen.contains(&entry.id),
            "duplicate anchor {} would make one question unlinkable",
            entry.id
        );
        assert!(
            entry
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{} is not a clean URL fragment",
            entry.id
        );
        seen.push(entry.id);
    }
}

#[test]
fn every_question_belongs_to_a_category_that_is_shown() {
    for entry in ENTRIES {
        assert!(
            FaqCategory::ALL.contains(&entry.category),
            "{} is filed under a category the page never renders",
            entry.id
        );
    }
}

#[test]
fn the_json_ld_is_valid_and_covers_every_question() {
    // Built by hand rather than through `serde_json`, so this checks the shape a
    // consumer actually parses: a `FAQPage` whose `mainEntity` is a Question per
    // entry, each with a non-empty `Answer`.
    let json = super::faq_json_ld(Locale::En);
    // Parsed rather than string-matched, because the shape is the contract: a
    // consumer reads `@type` and `acceptedAnswer`, so a JSON blob that merely
    // contains the right words in the wrong place is not a pass.
    let typed: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
    assert_eq!(text(&typed, "@type"), Some("FAQPage"));
    let entities = typed
        .get("mainEntity")
        .and_then(serde_json::Value::as_array)
        .map_or(&[][..], std::vec::Vec::as_slice);
    assert_eq!(entities.len(), ENTRIES.len(), "one entity per question");

    // The `@id` is what makes a question addressable from outside the page, so it
    // has to be the same fragment the heading renders.
    for (entry, entity) in ENTRIES.iter().zip(entities) {
        assert_eq!(
            text(entity, "@id"),
            Some(format!("#{}", entry.id).as_str()),
            "the @id must be the same fragment the heading renders"
        );
        assert_eq!(text(entity, "@type"), Some("Question"));

        let answer = entity.get("acceptedAnswer");
        let answer_type = answer.and_then(|answer| text(answer, "@type"));
        assert_eq!(answer_type, Some("Answer"));
        assert!(
            answer
                .and_then(|answer| text(answer, "text"))
                .is_some_and(|text| !text.is_empty()),
            "{} has an empty answer in the structured data",
            entry.id
        );
    }
}

#[test]
fn answers_are_short_enough_to_be_answers() {
    // A FAQ answer that runs long is documentation wearing a FAQ costume. The
    // long-form text already exists in `docs/`, linked from the answers that need
    // it, so anything past this is a sign of restating a document here.
    for entry in ENTRIES {
        assert!(
            entry.answer(Locale::En).chars().count() < 400,
            "{} is {} characters: that belongs in docs/, not an answer",
            entry.id,
            entry.answer(Locale::En).chars().count()
        );
    }
}

#[test]
fn the_page_furniture_is_translated_rather_than_left_in_english() {
    // The page's own words went in hardcoded while every answer under them was
    // translated, so a French or German reader got an English heading above twelve
    // translated answers. That is worse than an untranslated page: it looks like the
    // translation is broken rather than absent.
    for locale in LOCALES {
        let chrome = super::faq_chrome(locale);
        for (field, value) in [
            ("heading", chrome.heading),
            ("intro", chrome.intro),
            ("contents_label", chrome.contents_label),
            ("contents_heading", chrome.contents_heading),
        ] {
            assert!(
                !value.trim().is_empty(),
                "the FAQ {field} is empty in {locale:?}"
            );
        }
    }
    // Every locale differs from English somewhere. "FAQ" is deliberately identical
    // everywhere -- it is an initialism -- so it is not part of this.
    let english = super::faq_chrome(Locale::En);
    for locale in [Locale::Fr, Locale::De, Locale::It] {
        let chrome = super::faq_chrome(locale);
        assert_ne!(
            chrome.heading, english.heading,
            "untranslated heading in {locale:?}"
        );
        assert_ne!(
            chrome.intro, english.intro,
            "untranslated intro in {locale:?}"
        );
        assert_ne!(
            chrome.contents_heading, english.contents_heading,
            "untranslated contents heading in {locale:?}"
        );
    }
}

#[test]
fn entries_declare_the_locales_they_carry() {
    for entry in ENTRIES {
        let declared: Vec<Locale> = entry.translations.iter().map(|(l, _, _)| *l).collect();
        for locale in LOCALES {
            assert!(
                declared.contains(&locale),
                "{} does not declare {locale:?}",
                entry.id
            );
        }
        let _: &FaqEntry = entry;
    }
}
