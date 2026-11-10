// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use serde_json::Value;

// -- SPARQL / QS helpers -------------------------------------------------------

pub(super) use lotus_curation::qid_from_uri as extract_qid_from_uri;

pub(super) fn binding_value(binding: &Value, key: &str) -> Option<String> {
    binding
        .get(key)
        .and_then(|v| v.get("value"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

/// Escape a string literal for use inside a `QuickStatements` statement value.
/// Unlike SPARQL, `QuickStatements` string values (inside double quotes)
pub(super) fn escape_qs_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 4);
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}

/// Format a Wikidata `QuickStatements` mass statement using the dalton unit (Q483261). Unit
/// syntax is `U<QID>` - there is NO leading `Q` after the `U`.
/// Adds S887 reference "inferred from SMILES" (Q113907573).
pub fn qs_mass_statement(subject: &str, mass: f64) -> String {
    format!("{subject}|P2067|+{mass:.6}U483261|S887|Q113907573")
}

/// Reference QIDs for S887 (source/reference) in `QuickStatements`.
pub const QS_REF_INFERRED_FROM_SMILES: &str = "Q113907573"; // inferred from SMILES
pub const QS_REF_INFERRED_FROM_ISOMERIC_SMILES: &str = "Q123282952"; // inferred from isomeric SMILES

/// Build a `QuickStatements` statement with S887 reference(s).
/// Returns a statement like: `subject|prop|"value"|S887|Q113907573|S887|Q123282952`
pub fn qs_statement_with_refs(subject: &str, prop: &str, value: &str, refs: &[&str]) -> String {
    let mut stmt = format!("{subject}|{prop}|\"{}\"", escape_qs_string(value));
    for r in refs {
        stmt.push_str("|S887|");
        stmt.push_str(r);
    }
    stmt
}

/// Build a canonical SMILES statement with appropriate S887 references.
pub fn qs_canonical_smiles_statement(
    subject: &str,
    canonical_smiles: &str,
    has_isomeric: bool,
) -> String {
    if has_isomeric {
        qs_statement_with_refs(
            subject,
            "P233",
            canonical_smiles,
            &[crate::features::curation::services::helpers::QS_REF_INFERRED_FROM_ISOMERIC_SMILES],
        )
    } else {
        // No reference for canonical SMILES when no isomeric present
        format!("{subject}|P233|\"{}\"", escape_qs_string(canonical_smiles))
    }
}

/// Build an InChI statement with "inferred from SMILES" reference.
pub fn qs_inchi_statement(subject: &str, inchi: &str) -> String {
    qs_statement_with_refs(subject, "P234", inchi, &[QS_REF_INFERRED_FROM_SMILES])
}

/// Build an `InChIKey` statement with "inferred from SMILES" reference.
pub fn qs_inchikey_statement(subject: &str, inchikey: &str) -> String {
    qs_statement_with_refs(subject, "P235", inchikey, &[QS_REF_INFERRED_FROM_SMILES])
}

/// Build an isomeric SMILES statement WITHOUT any S887 reference.
/// Isomeric SMILES is the source itself, not inferred from something else.
pub fn qs_isomeric_smiles_statement(subject: &str, isomeric_smiles: &str) -> String {
    format!("{subject}|P2017|\"{}\"", escape_qs_string(isomeric_smiles))
}

// -- Text / chemistry normalization --------------------------------------------

pub(super) fn has_isomeric_smiles(smiles: &str) -> bool {
    lotus_curation::has_stereo_marks(smiles)
}

pub use lotus_curation::{extract_formula_from_inchi, normalize_formula_for_wikidata};

#[cfg(test)]
#[path = "helpers/tests.rs"]
mod tests;
