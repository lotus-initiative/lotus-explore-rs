// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Dependency `CREATE` blocks for missing Wikidata taxa and scholarly references.

use crate::{CurationInputRow, escape_quickstatements, extract_canonical_taxon_name, property};

/// The statements that would create a taxon the row needs and Wikidata lacks.
#[must_use]
pub fn taxon_dependency_statements(row: &CurationInputRow) -> Vec<String> {
    let Some(taxon) = row
        .taxon
        .as_deref()
        .map(str::trim)
        .filter(|taxon| !taxon.is_empty())
    else {
        return Vec::new();
    };
    let Some(canonical_taxon) = extract_canonical_taxon_name(taxon) else {
        return Vec::new();
    };
    vec![format!(
        "## {name} — taxon\nCREATE\n  LAST|{label}|\"{name}\"\n  LAST|{class}|{taxon_class}\n  LAST|{name_prop}|\"{name}\"",
        name = escape_quickstatements(&canonical_taxon),
        label = property::LABEL,
        class = "P31",
        taxon_class = crate::WD_TAXON_QID,
        name_prop = property::SCIENTIFIC_NAME,
    )]
}

/// The statements that would create a scholarly article item for a DOI the row
/// needs and Wikidata lacks.
#[must_use]
pub fn reference_dependency_statements(row: &CurationInputRow) -> Vec<String> {
    let Some(doi) = row.doi.as_deref().and_then(lotus_model::normalize_doi) else {
        return Vec::new();
    };
    let escaped = escape_quickstatements(&doi);
    vec![format!(
        "## {escaped} — reference\nCREATE\n  LAST|{label}|\"{escaped}\"\n  LAST|P31|Q13442814\n  LAST|{doi_prop}|\"{escaped}\"",
        label = property::LABEL,
        doi_prop = property::DOI,
    )]
}
