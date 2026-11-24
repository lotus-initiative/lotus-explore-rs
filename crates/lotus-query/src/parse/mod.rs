// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Reading `QLever`'s CSV into domain types.
//!
//! The parsers are deliberately forgiving. `flexible(true)` plus per-field
//! defaulting means a truncated or oddly-shaped payload degrades to fewer or
//! emptier columns rather than failing: a search that has already returned
//! usable rows should not be thrown away because one cell was unparsable.

mod columns;
mod compounds;
mod counts;
mod lookups;
mod stream;

#[cfg(test)]
mod tests;

pub use compounds::{parse_compounds_csv, parse_compounds_csv_capped};
pub use counts::parse_counts_csv;
pub use lookups::{
    CompoundMatch, ReferenceMatch, parse_compound_lookup_csv, parse_reference_lookup_csv,
    parse_taxon_csv,
};
pub use stream::{CsvColumnarReader, CsvSplitter, parse_compounds_columnar};

#[cfg(test)]
use lotus_model::TaxonNameSource;

use columns::{Columns, normalize_reference_node};
