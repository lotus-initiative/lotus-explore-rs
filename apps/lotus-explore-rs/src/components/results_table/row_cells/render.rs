// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Row-level render orchestration.

use crate::i18n::Locale;
use dioxus::prelude::*;
use lotus_model::CompoundEntry;
use std::sync::Arc;

use super::PreparedRow;
use super::cells::{
    compound_cell, formula_cell, mass_cell, reference_cell, structure_cell, taxon_cell, year_cell,
};
use super::row_text::RowText;

pub(in crate::components::results_table) use super::row_text::row_text;

#[component]
// `rows` and `prepared_rows` are the visible window and `keys` holds one key per row
// of it, so every index is in bounds.
//
// The window arrives already sliced and materialised: with the whole result set in
// memory, deriving a row means reading the columnar store and formatting a dozen
// strings, and doing that for three million rows to draw thirty would cost more than
// the whole store. `keys` are row offsets into the result set, not positions in the
// window, so a row keeps its DOM identity as the window scrolls.
#[allow(
    clippy::indexing_slicing,
    reason = "`keys` has one entry per row of `rows`/`prepared_rows`, so every index is within them"
)]
pub(in crate::components::results_table) fn ResultsRowsWindow(
    locale: Locale,
    text: RowText,
    rows: Arc<[CompoundEntry]>,
    prepared_rows: Arc<[PreparedRow]>,
    keys: Arc<[u32]>,
) -> Element {
    rsx! {
        for (i, key) in keys.iter().copied().enumerate() {
            if let (Some(row), Some(prepared)) = (rows.get(i), prepared_rows.get(i)) {
                {row_view(locale, text, row, prepared, key)}
            }
        }
    }
}

fn row_view(
    locale: Locale,
    text: RowText,
    entry: &CompoundEntry,
    prepared: &PreparedRow,
    row_key: u32,
) -> Element {
    let compound_qid = entry.compound_qid.as_ref();
    let taxon_qid = entry.taxon_qid.as_ref();
    let reference_qid = entry.reference_qid.as_ref();
    let name = prepared.display_name.as_ref();
    rsx! {
        tr {
            key: "{row_key}",
            "typeof": "ChemicalEntity",
            "about": "https://www.wikidata.org/entity/{compound_qid}",
            "data-lotus-id": "compound:{compound_qid}",
            // No `tabindex`: the row is not a control, so a focusable one is
            // just a dead tab stop. Hover tint stays (no cursor change); the
            // scroll container keeps its own `tabindex`.
            class: "data-row border-b border-shell-border hover:bg-surface/40 [contain:layout_paint]",
            {structure_cell(locale, text, prepared.depict_url.clone(), name)}
            {compound_cell(locale, text, entry, prepared, name, compound_qid)}
            {mass_cell(entry.mass)}
            {formula_cell(entry.formula.as_deref())}
            {taxon_cell(locale, text, entry, taxon_qid)}
            {reference_cell(locale, text, entry, prepared, reference_qid)}
            {year_cell(entry.pub_year)}
        }
    }
}
