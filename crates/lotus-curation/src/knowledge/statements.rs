// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `QuickStatements` statement generation for new and updated compounds/occurrences.

use std::fmt::Write as _;

use super::lookup::WikidataLookup;
use crate::{
    CurationInputRow, WikidataCompound, create_compound_statements, escape_quickstatements,
    property,
};

/// Shared note explaining why `status=revised` rows suppress automated writes.
pub const REVISED_ROW_NOTE: &str = "row marked status=revised; manual review required to deprecate or update the superseded Wikidata statement instead of creating a duplicate.";

/// Whether `row` marks a structural or biological revision (`status=revised`).
#[must_use]
pub fn is_revised_row(row: &CurationInputRow) -> bool {
    row.status
        .as_deref()
        .is_some_and(|s| s.trim().eq_ignore_ascii_case("revised"))
}

/// Comment block emitted in place of `CREATE`/`P703` statements for `status=revised` rows.
#[must_use]
pub fn revised_row_statement(row: &CurationInputRow) -> String {
    let name = escape_quickstatements(row.name.trim());
    format!(
        "## {name}\n# marked status=revised: automatic item creation and statement emission are suppressed. \
         Review the prior Wikidata item and deprecate or update the superseded statement manually."
    )
}

pub(super) fn is_reference_pending(row: &CurationInputRow, lookup: &WikidataLookup) -> bool {
    row.doi
        .as_deref()
        .and_then(lotus_model::normalize_doi)
        .is_some()
        && lookup.reference_qid.is_none()
}

fn occurrence_line(subject: &str, taxon: &str, reference: Option<&str>) -> String {
    let occurs = property::OCCURS_IN_TAXON;
    reference.map_or_else(
        || format!("{subject}|{occurs}|{taxon}"),
        |ref_qid| format!("{subject}|{occurs}|{taxon}|S248|{ref_qid}"),
    )
}

fn missing_compound_property_statements(
    compound: &WikidataCompound,
    converted: &crate::ConvertedStructure,
) -> Vec<String> {
    let mut lines = Vec::new();
    let qid = &compound.qid;

    if compound.canonical_smiles.is_none()
        && let Some(canonical) = converted
            .canonical_smiles
            .as_deref()
            .filter(|s| !s.trim().is_empty())
    {
        lines.push(format!(
            "{qid}|{prop}|\"{val}\"",
            prop = property::CANONICAL_SMILES,
            val = escape_quickstatements(canonical),
        ));
    }
    if compound.isomeric_smiles.is_none()
        && let Some(isomeric) = converted
            .isomeric_smiles
            .as_deref()
            .filter(|s| !s.trim().is_empty())
    {
        lines.push(format!(
            "{qid}|{prop}|\"{val}\"",
            prop = property::ISOMERIC_SMILES,
            val = escape_quickstatements(isomeric),
        ));
    }
    if compound.inchi.is_none()
        && let Some(inchi) = converted.inchi.as_deref().filter(|s| !s.trim().is_empty())
    {
        lines.push(format!(
            "{qid}|{prop}|\"{val}\"",
            prop = property::INCHI,
            val = escape_quickstatements(inchi),
        ));
    }
    if compound.formula.is_none()
        && let Some(formula) = converted.formula()
    {
        lines.push(format!(
            "{qid}|{prop}|\"{val}\"",
            prop = property::FORMULA,
            val = escape_quickstatements(&formula),
        ));
    }

    lines
}

/// The statements a row needs, and what curation concluded about it.
#[must_use]
pub fn statements_for(
    row: &CurationInputRow,
    converted: &crate::ConvertedStructure,
    lookup: &WikidataLookup,
) -> Vec<String> {
    if is_revised_row(row) {
        return vec![revised_row_statement(row)];
    }

    let name = escape_quickstatements(row.name.trim());
    let mut out = Vec::new();
    let ref_pending = is_reference_pending(row, lookup);

    let Some(compound) = &lookup.compound else {
        if converted.is_known() {
            let canonical = converted.canonical_smiles.as_deref().unwrap_or_default();
            let isomeric = converted.isomeric_smiles.as_deref().unwrap_or_default();
            let mut block = create_compound_statements(row.name.trim(), canonical, isomeric);
            if let Some(inchikey) = converted.inchikey.as_deref() {
                let _ = write!(
                    block,
                    "\n  LAST|{key}|\"{inchikey}\"",
                    key = property::INCHIKEY
                );
            }
            if let Some(inchi) = converted.inchi.as_deref().filter(|s| !s.trim().is_empty()) {
                let _ = write!(
                    block,
                    "\n  LAST|{prop}|\"{val}\"",
                    prop = property::INCHI,
                    val = escape_quickstatements(inchi),
                );
            }
            if let Some(formula) = converted.formula() {
                let _ = write!(
                    block,
                    "\n  LAST|{prop}|\"{val}\"",
                    prop = property::FORMULA,
                    val = escape_quickstatements(&formula),
                );
            }
            if let Some(taxon) = lookup.taxon_qid.as_deref()
                && !ref_pending
            {
                let line = occurrence_line("LAST", taxon, lookup.reference_qid.as_deref());
                let _ = write!(block, "\n  {line}");
            }
            out.push(block);
        } else {
            out.push(format!(
                "## {name}\n# no InChIKey for this structure, so no item can be \
                 created or matched. Check the structure in this row."
            ));
        }
        return out;
    };

    out.extend(missing_compound_property_statements(compound, converted));

    if let Some(taxon) = &lookup.taxon_qid
        && lookup.has_occurrence == Some(false)
        && !ref_pending
    {
        let statement = occurrence_line(&compound.qid, taxon, lookup.reference_qid.as_deref());
        out.push(format!(
            "## {name} — {qid} in {taxon}\n{statement}",
            qid = compound.qid,
        ));
    }

    out
}

/// The statements that would create a compound, for a row the run found missing.
#[must_use]
pub fn creation_statements(
    row: &CurationInputRow,
    canonical_smiles: &str,
    isomeric_smiles: &str,
) -> String {
    create_compound_statements(row.name.trim(), canonical_smiles, isomeric_smiles)
}
