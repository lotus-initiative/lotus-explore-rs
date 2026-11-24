// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! A result set stored by column, with the strings interned.
//!
//! # Why
//!
//! A [`CompoundEntry`] is right for one row, wrong for three million. Eight of its
//! thirteen fields are not row data: name, `InChIKey`, SMILES, mass and formula
//! belong to the *compound*, title, DOI and year to the *reference*, scientific
//! name to the *taxon*. Here a row is three dictionary ids and one statement id --
//! 32 bytes, measured by `a_row_costs_the_documented_number_of_bytes` -- against
//! roughly 200 bytes as a [`CompoundEntry`]. The table re-sorts and re-filters the
//! same set many times over.
//!
//! # Measurements
//!
//! From `QLever`, October 2026. Whole graph: 2,990,730 edges over 2,502,164
//! compounds, 2,818,725 statements, 37,771 taxa, 91,706 references. One export of
//! 50,000 rows in the columns the app selects: 15.7 MB CSV (314 B/row), 3.0 MB
//! gzipped (60 B/row).
//!
//! Two of those contradicted what this was first written expecting:
//!
//! - **`statement` does not compress.** It is `{compoundQID}-{UUID}` -- hyphen,
//!   not `$` -- and the UUID is random: 49,246 distinct statements in the sample
//!   had 49,246 distinct suffixes, so interning the string buys nothing. The
//!   redundant part is the prefix, which equals the row's own compound QID in
//!   49,983 of 50,000 rows. Hence [`StatementId::Uuid`], 16 bytes, text rebuilt on
//!   demand.
//! - **The compound dictionary barely deduplicates.** 2,990,730 edges over
//!   2,502,164 compounds is 1.2 rows per compound, so the dictionary is nearly one
//!   entry per row. The saving comes from the id columns and from **sparsity** --
//!   an `InChIKey` is present on 20.7% of rows, a SMILES on 15.0%, a mass on
//!   10.3%, a formula on 8.3% -- not from deduplication.
//!
//! # Cost
//!
//! Printed by `lotus-query/tests/bench.rs` via
//! `cargo test -p lotus-query --release -- --ignored --nocapture bench`. This
//! section quoted estimates for its first two commits and was wrong by more than a
//! factor of two, so only measured numbers appear here.
//!
//! | rows | resident | per row | as a share of the CSV |
//! | --- | --- | --- | --- |
//! | 1,000,000 | 84.1 MB | 88 B | 30% |
//!
//! Measured against the current projection columns (`ref_date` as year-only,
//! `ref_node`, and `compound_smiles_conn`):
//!
//! Scaling the measured rate: 2 million rows fit a 200 MB budget, and the whole
//! graph -- 2,990,730 rows -- comes to about 254 MB. That last figure is a floor,
//! because the benchmark fixture has two rows per compound where the real graph
//! has 1.2, so its compound dictionary is smaller than reality's.
//!
//! # Why this is not the phone limit
//!
//! 88 B/row is the cost of the *finished set*, and the parse never holds the CSV:
//! it reads 64 KB at a time into a fixed buffer. So on the streaming path peak is
//! the set plus one buffer, and the row count is bounded by device memory rather
//! than by payload size.
//!
//! The ceiling that *is* memory guards the last-resort export path -- the one
//! reached when a browser can neither stream to a file nor to private storage --
//! and it is denominated in bytes rather than rows, because a row costs 471 B as
//! CSV and roughly 1,500 B as Turtle. Compression does not lower it: a phone
//! decompresses the whole body into a string either way.
//!
//! # Exactness
//!
//! - **Absence is a first-class value.** [`NO_VALUE`] is an id no real dictionary
//!   entry can hold, so "no taxon" stays distinguishable from "taxon number
//!   zero". Collapsing them would silently change `n_taxa`.
//! - **Rows are stored raw.** The deduplication a [`CompoundEntry`] arrives
//!   *after* has not happened, so `n_entries` is the endpoint's `COUNT(*)` and not
//!   a lower bound on it.

mod access;
mod bitmask;
mod builder;
mod builder_seed;
#[cfg(feature = "diagnostics")]
mod diagnostics;
mod filter;
mod qid_dict;
mod sort;
mod sparse_strings;
mod statement_id;

#[cfg(test)]
mod tests;

#[cfg(test)]
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{CompoundEntry, DatasetStats};

pub use bitmask::Bitmask;
pub use builder::ColumnarBuilder;
pub use filter::{FilterPlan, FilterSpec, Range, contains_folded, folded};
pub use qid_dict::{QidDictionary, qid_text, write_qid};
pub use sparse_strings::SparseStrings;
pub use statement_id::StatementId;

#[cfg(test)]
use qid_dict::parse_numeric_qid;

/// Hands out each built set an identity that no other set shares.
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// A fresh identity for a set that has just been built.
///
/// Ids start at one so that the empty state, which keeps generation zero, can
/// never collide with a real result set.
fn next_generation() -> u64 {
    NEXT_GENERATION.fetch_add(1, Ordering::Relaxed)
}

/// The id that means "this row has no value here".
///
/// Handed out rather than interned, so no dictionary entry can hold it.
pub const NO_VALUE: u32 = u32::MAX;

/// The mass column's absence marker.
///
/// A mass is a measured quantity in daltons, so `NaN` cannot be one. That makes
/// it a free absence marker in a column over 90% absent, where `Option<f64>`
/// would cost eight bytes of padding per compound.
const NO_MASS: f64 = f64::NAN;

/// One row as it comes off the wire, before anything is interned.
///
/// Borrowed, so the CSV parser can hand over a record's fields without
/// allocating a `String` per field per row -- the largest avoidable cost in the
/// row-at-a-time path.
#[derive(Debug, Clone, Copy, Default)]
pub struct RawRow<'a> {
    /// Wikidata QID of the compound. Required.
    pub compound_qid: &'a str,
    /// Compound label; empty when there is none.
    pub name: &'a str,
    /// `InChIKey`, if the compound has one.
    pub inchikey: Option<&'a str>,
    /// SMILES, if the compound has one.
    pub smiles: Option<&'a str>,
    /// Monoisotopic mass in daltons, if the compound has one.
    pub mass: Option<f64>,
    /// Molecular formula, if the compound has one.
    pub formula: Option<&'a str>,
    /// Wikidata QID of the taxon; empty when there is none.
    pub taxon_qid: &'a str,
    /// Scientific name of the taxon; empty when there is none.
    pub taxon_name: &'a str,
    /// Wikidata QID of the reference; empty when there is none.
    pub reference_qid: &'a str,
    /// Wikidata QID of the reference *node* (`prov:wasDerivedFrom`), which is not
    /// the publication named by `reference_qid` via `pr:P248`. Empty when the
    /// occurrence has no reference. Kept for the provenance graph, not for display.
    pub reference_node: &'a str,
    /// Title of the reference, if it has one.
    pub ref_title: Option<&'a str>,
    /// DOI of the reference, if it has one.
    pub ref_doi: Option<&'a str>,
    /// Publication year, if the reference has one.
    pub pub_year: Option<i16>,
    /// Statement backing the occurrence, if there is one.
    pub statement: Option<&'a str>,
}

/// A result set in columnar form.
///
/// Built by [`ColumnarBuilder::build`]; there is no other way to make one, so a
/// set always has its statistics computed and its dictionaries consistent.
#[derive(Debug, Clone, Default)]
pub struct ColumnarResultSet {
    compound_ids: Vec<u32>,
    taxon_ids: Vec<u32>,
    reference_ids: Vec<u32>,
    statements: Vec<StatementId>,
    compounds: QidDictionary,
    compound_names: SparseStrings,
    compound_inchikeys: SparseStrings,
    compound_smiles: SparseStrings,
    compound_masses: Vec<f64>,
    compound_formulas: SparseStrings,
    taxa: QidDictionary,
    taxon_names: SparseStrings,
    references: QidDictionary,
    reference_titles: SparseStrings,
    reference_dois: SparseStrings,
    reference_years: Vec<Option<i16>>,
    statement_fallbacks: SparseStrings,
    /// Per-row reference-node hash, interned. Keyed by row, not by reference.
    reference_node_hashes: SparseStrings,
    stats: DatasetStats,
    /// Identifies this set, so it can be compared without being traversed.
    ///
    /// [`Default`] leaves this at **zero**, not one from the counter: every empty
    /// set means the same thing, and the reducer builds one to reset state, so a
    /// counter would make a freshly reset state look changed and re-render the
    /// table for nothing. Zero is not an id [`ColumnarBuilder::build`] hands out,
    /// so an empty set never equals a real one.
    generation: u64,
}

/// Two sets are equal when they are the same set.
///
/// A derived comparison would walk every row and dictionary -- the most expensive
/// thing in the program for a three-million-row set -- to answer a question the
/// caller can already answer, since it holds both. So this compares the identity
/// [`build`](ColumnarBuilder::build) stamped on each set: cheap, and exact in the
/// sense that matters, a new result set never equalling the one it replaced.
///
/// A clone keeps the identity, which is right: a clone is the same result.
impl PartialEq for ColumnarResultSet {
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation
    }
}

impl ColumnarResultSet {
    /// Build a set from rows that already exist as [`CompoundEntry`] values.
    ///
    /// For the CLI, for tests, and for anything holding a page rather than a
    /// stream. The statistics are the ones the stream path computes, which is
    /// what lets a test compare the two.
    #[must_use]
    pub fn from_entries(entries: &[CompoundEntry]) -> Self {
        let mut builder = ColumnarBuilder::new();
        for entry in entries {
            builder.push(RawRow {
                compound_qid: &entry.compound_qid,
                name: &entry.name,
                inchikey: entry.inchikey.as_deref(),
                smiles: entry.smiles.as_deref(),
                mass: entry.mass,
                formula: entry.formula.as_deref(),
                taxon_qid: &entry.taxon_qid,
                taxon_name: &entry.taxon_name,
                reference_qid: &entry.reference_qid,
                reference_node: &entry.reference_node,
                ref_title: entry.ref_title.as_deref(),
                ref_doi: entry.ref_doi.as_deref(),
                pub_year: entry.pub_year,
                statement: entry.statement.as_deref(),
            });
        }
        builder.build()
    }
}
