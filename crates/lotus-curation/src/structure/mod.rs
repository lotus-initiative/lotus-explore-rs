// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Turning a structure into the key Wikidata matches on.
//!
//! Wikidata identifies a compound by its `InChIKey` (`P235`); a curator's file has
//! `SMILES`. The conversion goes to the same public API the web client already falls
//! back to when deriving canonical/isomeric SMILES, `InChIKey`, and `InChI`.

mod convert;
mod formula;

#[cfg(test)]
mod tests;

pub use convert::{convert_structure, convert_structures};
pub use formula::{
    extract_formula_from_inchi, has_stereo_marks, normalize_formula_for_wikidata,
    strip_stereo_marks,
};

#[cfg(test)]
use crate::CurationError;
#[cfg(test)]
use convert::Converted;

/// A structure, in the forms Wikidata and a `QuickStatements` bundle want.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConvertedStructure {
    /// The `InChIKey`, which is how the compound is looked up.
    pub inchikey: Option<String>,
    /// Canonical `SMILES`, which drops stereochemistry.
    pub canonical_smiles: Option<String>,
    /// Isomeric `SMILES`, which keeps it.
    pub isomeric_smiles: Option<String>,
    /// The `InChI`.
    pub inchi: Option<String>,
}

impl ConvertedStructure {
    /// The lookup key, if the conversion produced one.
    #[must_use]
    pub fn structure_key(&self) -> crate::StructureKey {
        crate::StructureKey {
            inchikey: self.inchikey.clone(),
        }
    }

    /// Whether the conversion produced anything usable.
    #[must_use]
    pub fn is_known(&self) -> bool {
        self.inchikey
            .as_deref()
            .is_some_and(|key| !key.trim().is_empty())
    }

    /// The Wikidata-formatted molecular formula (`P274`) implied by `self.inchi`,
    /// when `inchi` has a formula layer.
    #[must_use]
    pub fn formula(&self) -> Option<String> {
        self.inchi
            .as_deref()
            .and_then(extract_formula_from_inchi)
            .map(|f| normalize_formula_for_wikidata(&f))
    }
}
