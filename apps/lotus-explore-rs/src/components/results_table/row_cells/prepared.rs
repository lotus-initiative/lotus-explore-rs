// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use lotus_model::CompoundEntry;
use std::sync::Arc;

#[derive(Clone, PartialEq, Debug)]
pub(in crate::components::results_table) struct PreparedRow {
    pub(super) display_name: Arc<str>,
    pub(super) depict_url: Option<Arc<str>>,
    pub(super) doi: Option<Arc<str>>,
    pub(super) statement_id: Option<Arc<str>>,
}

impl PreparedRow {
    /// Derive the row's display fields from one materialised row.
    ///
    /// Called only for the rows on screen. The previous shape pre-derived one per row
    /// for the whole result set, including URL-encoding a SMILES into a CDK depict link
    /// — a hundred-odd bytes per row, for the thirty a viewport holds.
    pub(in crate::components::results_table) fn from_entry(entry: &CompoundEntry) -> Self {
        Self {
            display_name: normalized_display_name(entry),
            depict_url: depict_url_cached(entry),
            doi: trimmed_optional_arc(entry.ref_doi.as_ref()),
            statement_id: trimmed_statement_id_arc(entry.statement.as_ref()),
        }
    }
}

fn normalized_display_name(entry: &CompoundEntry) -> Arc<str> {
    let trimmed = entry.name.trim();
    if trimmed.is_empty() {
        entry.compound_qid.clone()
    } else if trimmed.len() == entry.name.len() {
        entry.name.clone()
    } else {
        Arc::<str>::from(trimmed)
    }
}

fn depict_url_cached(entry: &CompoundEntry) -> Option<Arc<str>> {
    let smiles = entry.smiles.as_deref()?.trim();
    if smiles.is_empty() || smiles.contains('\n') {
        return None;
    }
    let encoded = lotus_model::percent_encode(smiles);
    let mut url = String::with_capacity(
        "https://www.simolecule.com/cdkdepict/depict/cow/svg?smi=".len()
            + encoded.len()
            + "&annotate=cip".len(),
    );
    url.push_str("https://www.simolecule.com/cdkdepict/depict/cow/svg?smi=");
    url.push_str(encoded.as_ref());
    url.push_str("&annotate=cip");
    Some(Arc::<str>::from(url))
}

fn trimmed_optional_arc(value: Option<&Arc<str>>) -> Option<Arc<str>> {
    let arc = value?;
    let trimmed = arc.trim();
    if trimmed.is_empty() {
        None
    } else if trimmed.len() == arc.len() {
        Some(Arc::clone(arc))
    } else {
        Some(Arc::<str>::from(trimmed))
    }
}

fn trimmed_statement_id_arc(value: Option<&Arc<str>>) -> Option<Arc<str>> {
    const STMT_PREFIX: &str = "http://www.wikidata.org/entity/statement/";
    let arc = value?;
    let trimmed = arc.trim();
    if trimmed.is_empty() {
        return None;
    }
    let stripped = trimmed.strip_prefix(STMT_PREFIX).unwrap_or(trimmed);
    if stripped.len() == arc.len() {
        Some(Arc::clone(arc))
    } else {
        Some(Arc::<str>::from(stripped))
    }
}

#[cfg(test)]
#[path = "prepared/tests.rs"]
mod tests;
