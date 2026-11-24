// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Per-dictionary memory accounting behind the `diagnostics` feature.

use std::sync::Arc;

use super::{ColumnarResultSet, QidDictionary, SparseStrings};

impl ColumnarResultSet {
    /// What each dictionary costs, so an optimisation can be aimed at the largest
    /// one rather than at the most obvious.
    ///
    /// Read by `lotus-query/tests/bench.rs`, so behind the `diagnostics`
    /// feature -- see the note on it in `Cargo.toml`.
    ///
    /// Returns `(label, entries, bytes)`. `bytes` counts the string data, one fat
    /// pointer per string and one slot per map entry: what the structure holds,
    /// not what the allocator reserved.
    #[must_use]
    pub fn dictionary_costs(&self) -> Vec<(&'static str, usize, usize)> {
        const PTR: usize = std::mem::size_of::<Arc<str>>();
        // A QID dictionary holds integers: a key and a position, plus the map's own
        // slack, and no string at all. That is the point of the type.
        let qids = |d: &QidDictionary| d.len() * (std::mem::size_of::<u32>() * 2 + 8);
        let sparse = |s: &SparseStrings| {
            s.values.len() * PTR
                + s.values.iter().map(|v| v.len()).sum::<usize>()
                + s.ids.len() * std::mem::size_of::<u32>()
        };
        vec![
            ("compound qids", self.compounds.len(), qids(&self.compounds)),
            (
                "compound names",
                self.compound_names.len(),
                sparse(&self.compound_names),
            ),
            (
                "compound inchikeys",
                self.compound_inchikeys.len(),
                sparse(&self.compound_inchikeys),
            ),
            (
                "compound smiles",
                self.compound_smiles.len(),
                sparse(&self.compound_smiles),
            ),
            (
                "compound formulas",
                self.compound_formulas.len(),
                sparse(&self.compound_formulas),
            ),
            (
                "compound masses",
                self.compound_masses.len(),
                self.compound_masses.len() * 8,
            ),
            ("taxon qids", self.taxa.len(), qids(&self.taxa)),
            (
                "taxon names",
                self.taxon_names.len(),
                sparse(&self.taxon_names),
            ),
            (
                "reference qids",
                self.references.len(),
                qids(&self.references),
            ),
            (
                "reference titles",
                self.reference_titles.len(),
                sparse(&self.reference_titles),
            ),
            (
                "reference dois",
                self.reference_dois.len(),
                sparse(&self.reference_dois),
            ),
            (
                "reference years",
                self.reference_years.len(),
                self.reference_years.len() * 4,
            ),
            // Not `len()`: this table is interned by id and never fills the
            // slot column `len()` counts, so that would always read zero.
            (
                "statement fallbacks",
                self.statement_fallbacks.distinct_len(),
                sparse(&self.statement_fallbacks),
            ),
        ]
    }

    /// The bytes the dictionaries hold, for a measurement rather than for
    /// anything the app needs: the sum of [`Self::dictionary_costs`].
    ///
    /// One table and its sum, rather than a second estimate written over the same
    /// fields: two estimates drifted apart, and nothing in the build could tell.
    /// Not a measurement of the allocator: on wasm that reports the module's
    /// high-water mark rather than what this type holds.
    #[must_use]
    pub fn total_dictionary_bytes(&self) -> usize {
        self.dictionary_costs()
            .iter()
            .map(|(_, _, bytes)| bytes)
            .sum()
    }
}
