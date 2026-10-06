// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `row_text`, in their own file.

#![allow(clippy::indexing_slicing)]

use super::*;
use crate::i18n::Locale;

#[test]
fn all_row_text_fields_are_non_empty_for_every_locale() {
    for locale in [Locale::En, Locale::Fr, Locale::De, Locale::It] {
        let text = row_text(locale);
        assert!(
            !text.open_full_size_depiction.is_empty(),
            "locale {locale:?}: open_full_size_depiction"
        );
        assert!(
            !text.open_in_wikidata.is_empty(),
            "locale {locale:?}: open_in_wikidata"
        );
        assert!(
            !text.open_in_scholia.is_empty(),
            "locale {locale:?}: open_in_scholia"
        );
        assert!(!text.open_doi.is_empty(), "locale {locale:?}: open_doi");
        assert!(!text.statement.is_empty(), "locale {locale:?}: statement");
    }
}

#[test]
fn row_text_fields_are_pairwise_distinct_for_default_locale() {
    let text = row_text(Locale::En);
    let fields = [
        text.open_full_size_depiction,
        text.open_in_wikidata,
        text.open_in_scholia,
        text.open_doi,
        text.statement,
    ];
    for i in 0..fields.len() {
        for j in (i + 1)..fields.len() {
            assert_ne!(
                fields[i], fields[j],
                "text fields at indices {i} and {j} should be distinct"
            );
        }
    }
}
