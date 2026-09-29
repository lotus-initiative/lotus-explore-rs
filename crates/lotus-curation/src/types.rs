// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// One line of a curation input: what to look up, and what it is called.
pub struct CurationInputRow {
    /// The name the curator supplied.
    pub name: String,
    /// The structure, as `SMILES` or a molfile.
    pub smiles: String,
    /// The reporting organism, if the row names one.
    pub taxon: Option<String>,
    /// The reference `DOI`, if the row cites one.
    pub doi: Option<String>,
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

#[derive(Clone, Debug, PartialEq, Serialize)]
/// One row after curation: what `Wikidata` has, and what it would take to add it.
pub struct CurationResultRow {
    /// The row this result came from.
    pub input: CurationInputRow,
    /// The structure after canonicalisation.
    pub canonical_smiles: Option<String>,
    /// The `InChIKey`, which is how the same molecule is recognised across sources.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Which part of curation failed, for deciding whether to retry.
pub enum CurationErrorKind {
    /// The file or a row could not be read.
    InvalidInput,
    /// The request failed.
    Transport,
    /// The answer arrived but could not be read.
    Parse,
}

#[derive(Debug, Error)]
/// A curation failure, at the layer where it can still be classified.
pub enum CurationError {
    /// The input was not usable, with the reason.
    #[error("{0}")]
    InvalidInput(String),
    /// A required column was absent, named so the user can add it.
    #[error("TSV is missing a '{0}' column")]
    MissingTsvColumn(&'static str),
    /// Wikidata or the natural-products API could not be reached.
    #[error("{0}")]
    Http(String),
    /// An answer arrived in a shape this crate does not read.
    #[error("{0}")]
    Parse(String),
}

impl CurationError {
    /// Which part of curation failed, for deciding whether to retry.
    #[must_use]
    pub const fn kind(&self) -> CurationErrorKind {
        match self {
            Self::InvalidInput(_) | Self::MissingTsvColumn(_) => CurationErrorKind::InvalidInput,
            Self::Http(_) => CurationErrorKind::Transport,
            Self::Parse(_) => CurationErrorKind::Parse,
        }
    }

    /// Whether trying again with the same input could plausibly work.
    ///
    /// Only a transport failure is worth retrying, and only because the input was
    /// never in question. Two other failures are final and retrying them just
    /// spends the endpoint's rate limit:
    ///
    /// - a parse failure, because the answer is there and is not in a shape this
    ///   crate reads, so a second request would be refused the same way;
    /// - an input failure, because the same malformed structure is the same
    ///   malformed structure.
    #[must_use]
    pub const fn is_recoverable(&self) -> bool {
        matches!(self, Self::Http(_))
    }
}
