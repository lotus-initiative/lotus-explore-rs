// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Assembling a [`CurationResultRow`] from input, structure conversion, and Wikidata lookup state.

use super::dependencies::{reference_dependency_statements, taxon_dependency_statements};
use super::lookup::WikidataLookup;
use super::statements::{REVISED_ROW_NOTE, is_reference_pending, is_revised_row, statements_for};
use crate::{CurationInputRow, CurationResultRow, CurationStatus, is_binomial};

/// Turn a lookup into a result row.
#[must_use]
pub fn to_result_row(
    row: &CurationInputRow,
    converted: &crate::ConvertedStructure,
    lookup: &WikidataLookup,
) -> CurationResultRow {
    let key = converted.structure_key();
    let statements = statements_for(row, converted, lookup);

    let taxon_named = row
        .taxon
        .as_deref()
        .map(str::trim)
        .is_some_and(|taxon| !taxon.is_empty());
    let taxon_pending = taxon_named && lookup.taxon_qid.is_none();
    let ref_pending = is_reference_pending(row, lookup);
    let deps_pending = taxon_pending || ref_pending;

    let status = if !key.is_known() {
        CurationStatus::Error
    } else if is_revised_row(row) {
        CurationStatus::ExistingNeedsUpdates
    } else if deps_pending {
        CurationStatus::PendingDependencies
    } else if lookup.compound.is_some() {
        if statements.is_empty() {
            CurationStatus::ExistingComplete
        } else {
            CurationStatus::ExistingNeedsUpdates
        }
    } else {
        CurationStatus::NewCompound
    };

    let note = build_note(row, lookup, key.is_known(), ref_pending);

    let mut dependency_blocks = Vec::new();
    if !is_revised_row(row) {
        if taxon_pending {
            dependency_blocks.extend(taxon_dependency_statements(row));
        }
        if ref_pending {
            dependency_blocks.extend(reference_dependency_statements(row));
        }
    }

    CurationResultRow {
        input: row.clone(),
        canonical_smiles: converted.canonical_smiles.clone().or_else(|| {
            lookup
                .compound
                .as_ref()
                .and_then(|c| c.canonical_smiles.clone())
        }),
        isomeric_smiles: converted.isomeric_smiles.clone().or_else(|| {
            lookup
                .compound
                .as_ref()
                .and_then(|c| c.isomeric_smiles.clone())
        }),
        inchikey: key.inchikey,
        inchi: converted
            .inchi
            .clone()
            .or_else(|| lookup.compound.as_ref().and_then(|c| c.inchi.clone())),
        formula: lookup
            .compound
            .as_ref()
            .and_then(|c| c.formula.clone())
            .or_else(|| converted.formula()),
        exact_mass: lookup.compound.as_ref().and_then(|c| c.mass),
        mass_warning: None,
        wikidata_qid: lookup.compound.as_ref().map(|c| c.qid.clone()),
        status,
        note,
        dependency_blocks,
        quickstatements: statements,
    }
}

fn build_note(
    row: &CurationInputRow,
    lookup: &WikidataLookup,
    key_known: bool,
    ref_pending: bool,
) -> String {
    let mut note = String::new();
    if is_revised_row(row) {
        note.push_str(REVISED_ROW_NOTE);
        note.push(' ');
    }
    if !key_known {
        note.push_str("no InChIKey for this structure; nothing could be looked up. ");
    }
    if let Some(taxon) = row
        .taxon
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        if !is_binomial(taxon) {
            if taxon.contains(char::is_whitespace) {
                note.push_str(
                    "the taxon uses open nomenclature or lacks a specific epithet, so it cannot be resolved unambiguously. ",
                );
            } else {
                note.push_str("the taxon is a genus on its own, which is ambiguous. ");
            }
        } else if lookup.taxon_qid.is_none() {
            note.push_str("Wikidata has no taxon by that name. ");
        }
        if row
            .doi
            .as_deref()
            .and_then(lotus_model::normalize_doi)
            .is_none()
        {
            note.push_str(
                "no DOI provided; occurrence statement has no literature reference (P248). ",
            );
        }
    }
    if ref_pending {
        note.push_str("Wikidata has no reference with that DOI. ");
    }
    note.trim().to_string()
}
