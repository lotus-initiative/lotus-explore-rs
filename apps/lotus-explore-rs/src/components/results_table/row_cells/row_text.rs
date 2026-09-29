// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Locale-resolved text bundle for results-table row rendering.

use crate::i18n::{Locale, TextKey, t};

/// All static text strings needed to render a single results-table row.
/// Resolved once per render from the active locale and cheaply copied into
/// each `row_view` call, avoiding repeated locale lookups per cell.
#[derive(Clone, Copy, PartialEq)]
pub(in crate::components::results_table) struct RowText {
    pub(super) open_full_size_depiction: &'static str,
    pub(super) open_in_wikidata: &'static str,
    pub(super) open_in_scholia: &'static str,
    pub(super) open_doi: &'static str,
    pub(super) statement: &'static str,
}

/// Build a `RowText` bundle for the given locale.
#[must_use]
pub(in crate::components::results_table) fn row_text(locale: Locale) -> RowText {
    RowText {
        open_full_size_depiction: t(locale, TextKey::OpenFullSizeDepiction),
        open_in_wikidata: t(locale, TextKey::OpenInWikidata),
        open_in_scholia: t(locale, TextKey::OpenInScholia),
        open_doi: t(locale, TextKey::OpenDoi),
        statement: t(locale, TextKey::Statement),
    }
}

#[cfg(test)]
mod tests {
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
}
