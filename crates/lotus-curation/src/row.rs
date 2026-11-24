// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Curation input rows, outcome statuses, result rows, and `QuickStatements` bundles.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// One line of a curation input: what to look up, and what it is called.
pub struct CurationInputRow {
    /// The name the curator supplied.
    pub name: String,
    /// The structure, as `SMILES` or a molfile.
    pub smiles: String,
    /// The reporting taxon, if the row names one.
    pub taxon: Option<String>,
    /// The reference `DOI`, if the row cites one.
    pub doi: Option<String>,
    /// Optional curation status (e.g., `revised`, `confirmed`, `new`).
    /// Rows marked `revised` suppress automatic `CREATE` statement emission so
    /// a curator can deprecate or update the prior Wikidata claim manually.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
/// What curation concluded about a row.
pub enum CurationStatus {
    /// Wikidata already says everything this row would.
    ExistingComplete,
    /// Wikidata has the item but is missing something this row knows.
    ExistingNeedsUpdates,
    /// No such item in Wikidata; these statements would create it.
    NewCompound,
    /// A taxon or reference the row needs has not been found yet.
    PendingDependencies,
    /// The row was not looked up, so nothing is known about it.
    ///
    /// Not the same as "new". A run that was asked not to touch the network, or
    /// that was cut short, has not established that a compound is absent -- and
    /// reporting it as absent is how a duplicate gets submitted. This state
    /// exists so that "I did not look" is sayable.
    NotChecked,
    /// Curation could not be completed for this row.
    Error,
}

impl CurationStatus {
    /// Stable short status key shared by CLI reports and UI i18n lookup.
    #[must_use]
    pub const fn as_key(&self) -> &'static str {
        match self {
            Self::ExistingComplete => "existing_complete",
            Self::ExistingNeedsUpdates => "existing_updates",
            Self::NewCompound => "new_compound",
            Self::PendingDependencies => "pending_dependencies",
            Self::NotChecked => "not_checked",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
/// One row after curation: what `Wikidata` has, and what it would take to add it.
pub struct CurationResultRow {
    /// The row this result came from.
    pub input: CurationInputRow,
    /// The structure after canonicalisation (`P233`, stereo-stripped).
    pub canonical_smiles: Option<String>,
    /// The stereochemistry-preserving `SMILES` (`P2017`), when the structure has stereochemistry.
    pub isomeric_smiles: Option<String>,
    /// The `InChIKey` (`P235`), which is how the same molecule is recognised across sources.
    pub inchikey: Option<String>,
    /// The `InChI`.
    pub inchi: Option<String>,
    /// The molecular formula.
    pub formula: Option<String>,
    /// The canonicalised exact mass.
    pub exact_mass: Option<f64>,
    /// A caveat about the mass, if there is one.
    pub mass_warning: Option<String>,
    /// The item this row resolved to, if it resolved.
    pub wikidata_qid: Option<String>,
    /// What curation concluded.
    pub status: CurationStatus,
    /// A sentence for a human, about anything surprising.
    pub note: String,
    /// This row's dependencies, for bundling.
    pub dependency_blocks: Vec<String>,
    /// The statements for this row alone, in dependency order.
    pub quickstatements: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Statements ready to paste into a `QuickStatements` editor, split so that
/// dependencies can be run to completion before the rows that need them.
pub struct QuickStatementsBundle {
    /// Dependency statements, deduplicated, to run first.
    pub dependencies: std::sync::Arc<str>,
    /// The rows' own statements.
    pub main: std::sync::Arc<str>,
}

impl Default for QuickStatementsBundle {
    fn default() -> Self {
        Self {
            dependencies: std::sync::Arc::<str>::from(""),
            main: std::sync::Arc::<str>::from(""),
        }
    }
}

#[cfg(test)]
#[path = "row/tests.rs"]
mod tests;
