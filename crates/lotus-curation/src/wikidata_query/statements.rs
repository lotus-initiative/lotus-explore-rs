// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Wikidata curation property constants, QID/SPARQL escaping helpers, and item creation blocks.

use crate::{WD_CHEMICAL_COMPOUND_QID, WD_TYPE_CHEMICAL_ENTITY_QID};

/// The Wikidata properties curation reads and writes.
pub mod property {
    /// Canonical SMILES.
    pub const CANONICAL_SMILES: &str = "P233";
    /// Isomeric SMILES, which preserves stereochemistry.
    pub const ISOMERIC_SMILES: &str = "P2017";
    /// `InChIKey`, the identifier a structure is matched on.
    pub const INCHIKEY: &str = "P235";
    /// `InChI`.
    pub const INCHI: &str = "P234";
    /// Molecular formula.
    pub const FORMULA: &str = "P274";
    /// Exact mass.
    pub const EXACT_MASS: &str = "P2067";
    /// English label in `QuickStatements` syntax.
    pub const LABEL: &str = "Len";
    /// "Found in taxon".
    pub const OCCURS_IN_TAXON: &str = "P703";
    /// DOI, the identifier a reference is matched on.
    pub const DOI: &str = "P356";
    /// Scientific name, as a taxon item is named.
    pub const SCIENTIFIC_NAME: &str = "P225";
}

/// Escape a value for a SPARQL string literal.
#[must_use]
pub fn escape_sparql_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' | '\t' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// The QID out of a Wikidata entity URI, or `None` for anything else.
#[must_use]
pub fn qid_from_uri(value: &str) -> Option<&str> {
    value.rsplit('/').next().filter(|last| {
        last.starts_with('Q') && last.len() > 1 && last[1..].chars().all(|c| c.is_ascii_digit())
    })
}

/// Numeric value of a Wikidata QID (`Q20` -> `20`), or `u64::MAX` for anything
/// non-numeric so valid QIDs always sort before malformed placeholders.
#[must_use]
pub fn qid_number(qid: &str) -> u64 {
    qid.strip_prefix('Q')
        .and_then(|n| n.parse().ok())
        .unwrap_or(u64::MAX)
}

/// The statements that would add a compound to Wikidata, given a structure.
#[must_use]
pub fn create_compound_statements(
    name: &str,
    canonical_smiles: &str,
    isomeric_smiles: &str,
) -> String {
    use std::fmt::Write as _;
    let mut out = format!(
        "CREATE\n  LAST|{label}|\"{name}\"\n  \
         LAST|P31|{entity_type}\n  \
         LAST|P279|{compound_class}\n  \
         LAST|{canonical}|\"{canonical_value}\"",
        label = property::LABEL,
        name = crate::escape_quickstatements(name),
        entity_type = WD_TYPE_CHEMICAL_ENTITY_QID,
        compound_class = WD_CHEMICAL_COMPOUND_QID,
        canonical = property::CANONICAL_SMILES,
        canonical_value = crate::escape_quickstatements(canonical_smiles),
    );
    if !isomeric_smiles.trim().is_empty() {
        let _ = write!(
            out,
            "\n  LAST|{isomeric}|\"{isomeric_value}\"",
            isomeric = property::ISOMERIC_SMILES,
            isomeric_value = crate::escape_quickstatements(isomeric_smiles),
        );
    }
    out
}
