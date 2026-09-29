// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! LOTUS domain types and filter semantics.
//!
//! Everything here is pure: no IO, no async, no clock, no platform. A caller
//! that needs the current year or the current time passes it in, which is why
//! this crate compiles for `wasm32-unknown-unknown` and is testable without a
//! runtime.
//!
//! The vocabulary: a **result row** ([`CompoundEntry`]) is one
//! compound-found-in-taxon-cited-by-reference triple, which is the shape the
//! LOTUS projection in Wikidata produces.

#![warn(missing_docs)]

mod criteria;
mod entry;
mod identify;
mod stats;
mod structure;
mod validate;

pub use criteria::SearchCriteria;
pub use entry::{CompoundEntry, Rows, TaxonMatch};
pub use identify::{non_empty, normalize_doi, normalize_qid};
pub use stats::{DatasetStats, ElementState, SmilesSearchType};
pub use structure::{StructureKind, classify_structure};
pub use validate::{ValidationError, validate_criteria};

/// Base URI for Wikidata entities (`Q123` → `<BASE>Q123`).
pub const WIKIDATA_ENTITY_BASE: &str = "http://www.wikidata.org/entity/";

/// Base URI for Wikidata reification statements (`S1` → `<BASE>statement/S1`).
pub const WIKIDATA_STATEMENT_BASE: &str = "http://www.wikidata.org/entity/statement/";

/// Widest atom count each of the six filterable elements is given by default.
///
/// A count above these cannot correspond to a real molecule, so a filter that
/// asks for one is rejected rather than used to widen the result set.
pub mod element_max {
    /// Carbon.
    pub const C: u16 = 512;
    /// Hydrogen.
    pub const H: u16 = 1_024;
    /// Nitrogen.
    pub const N: u16 = 256;
    /// Oxygen.
    pub const O: u16 = 256;
    /// Phosphorus.
    pub const P: u16 = 128;
    /// Sulfur.
    pub const S: u16 = 64;
}

/// Upper bound for the mass range, in daltons.
pub const MASS_MAX: f64 = 10_000.0;

/// Earliest plausible publication year. Earlier means a parsing fault, not
/// history: the LOTUS sources are modern.
pub const YEAR_MIN: u16 = 1800;

/// Longest taxon string accepted, in bytes.
pub const TAXON_MAX_LEN: usize = 500;

/// Longest structure string accepted, in bytes.
pub const STRUCTURE_MAX_LEN: usize = 10_000;
