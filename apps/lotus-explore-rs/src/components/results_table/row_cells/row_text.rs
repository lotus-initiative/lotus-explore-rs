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
#[path = "row_text/tests.rs"]
mod tests;
