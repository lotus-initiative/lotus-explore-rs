// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Row-level render orchestration.

use crate::i18n::Locale;
use crate::models::CompoundEntry;
use dioxus::prelude::*;
use std::sync::Arc;

use super::PreparedRow;
use super::cells::{
    compound_cell, formula_cell, mass_cell, reference_cell, structure_cell, taxon_cell, year_cell,
};
use super::row_text::RowText;

pub(in crate::components::results_table) use super::row_text::row_text;

#[component]
// `end` is clamped to `order.len()`, and `i` is drawn from `order`, whose
// entries are row offsets into the same `rows`/`prepared_rows` arrays, so
// every index here stays in bounds.
#[allow(clippy::indexing_slicing)]
pub(in crate::components::results_table) fn ResultsRowsWindow(
    locale: Locale,
    text: RowText,
    rows: Arc<[CompoundEntry]>,
    prepared_rows: Arc<[PreparedRow]>,
    order: Arc<[u32]>,
    start_row: usize,
    end_row: usize,
) -> Element {
    let start = start_row.min(order.len());
    let end = end_row.min(order.len()).max(start);
    rsx! {
        for i in order[start..end].iter().copied() {
            {row_view(locale, text, &rows[i as usize], &prepared_rows[i as usize], i)}
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
            // No `tabindex` and no focus ring: the row is not a control. It has
            // no click handler and no interactive role, so making it focusable
            // only put 13 dead stops on the search page (growing with the
            // virtualised table) that a keyboard user tabs through to reach
            // nothing, which Firefox reports as "Clickable elements must be
            // focusable and should have interactive semantics" and as failing
            // keyboard accessibility. The hover tint stays because it is a
            // scanning aid and does not change the cursor. The scroll container
            // keeps its own `tabindex`, which is what actually needs to be
            // focusable.
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
