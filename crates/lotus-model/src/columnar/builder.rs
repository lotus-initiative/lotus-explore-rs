// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Incremental builder that interns streamed occurrence rows and metadata into [`ColumnarResultSet`].

use rustc_hash::{FxBuildHasher, FxHashSet};

use super::bitmask::Bitmask;
use super::builder_seed::unseeded_qids;
use super::qid_dict::{intern_qid, lookup_slot};
use super::statement_id::{ParsedStatement, StatementId, strip_statement_prefix};
use super::{ColumnarResultSet, DatasetStats, NO_MASS, NO_VALUE, RawRow, next_generation};

/// Assembles a [`ColumnarResultSet`].
///
/// The dictionaries grow as rows arrive, which is what lets the CSV parser hand
/// over records without knowing the final size first. Statistics are taken once,
/// in [`build`](Self::build), because taking them per row would cost a pass over
/// the whole set for every row that arrived.
#[derive(Debug, Default)]
pub struct ColumnarBuilder {
    pub(super) set: ColumnarResultSet,
    pub(super) seeded_compounds: Bitmask,
    pub(super) seeded_references: Bitmask,
}

impl ColumnarBuilder {
    /// A builder holding nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many rows have been pushed.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.set.compound_ids.len()
    }

    /// Whether neither compound nor reference dictionaries hold any entries.
    #[must_use]
    pub const fn is_empty_dictionaries(&self) -> bool {
        self.set.compounds.is_empty() && self.set.references.is_empty()
    }

    /// Every distinct compound QID interned so far that has not yet been
    /// seeded from a previous set, in first-seen order.
    pub fn compound_qids(&self) -> impl Iterator<Item = u32> + '_ {
        unseeded_qids(&self.set.compounds, &self.seeded_compounds)
    }

    /// Every distinct reference QID interned so far that has not yet been
    /// seeded from a previous set, in first-seen order.
    pub fn reference_qids(&self) -> impl Iterator<Item = u32> + '_ {
        unseeded_qids(&self.set.references, &self.seeded_references)
    }

    /// Intern one row and record its ids.
    ///
    /// A row with no compound QID is dropped: there would be nothing to key it
    /// on. This is the only place a row can be lost, and it is why `n_entries`
    /// can still disagree with the endpoint if the endpoint counts a row this
    /// cannot represent.
    pub fn push(&mut self, row: RawRow<'_>) {
        let Some(compound_id) = intern_qid(&mut self.set.compounds, row.compound_qid) else {
            return;
        };
        let compound_slot = compound_id as usize;
        self.grow_compounds();
        let numeric = self.set.compounds.get(compound_id).unwrap_or_default();
        self.populate_compound(compound_slot, row);

        let taxon_id = intern_qid(&mut self.set.taxa, row.taxon_qid).map_or(NO_VALUE, |id| {
            self.set
                .taxon_names
                .set(id as usize, crate::non_empty(row.taxon_name));
            id
        });
        let reference_id =
            intern_qid(&mut self.set.references, row.reference_qid).map_or(NO_VALUE, |id| {
                self.grow_references();
                self.populate_reference(id as usize, row);
                id
            });

        let statement =
            row.statement
                .map(strip_statement_prefix)
                .map_or(StatementId::Absent, |cell| {
                    match StatementId::parse(cell, numeric) {
                        ParsedStatement::Absent => StatementId::Absent,
                        ParsedStatement::Uuid(bytes) => StatementId::Uuid(bytes),
                        ParsedStatement::Other => {
                            // No slot, and no prediction: the id comes from the interning table.
                            // This column is a plain id -> string map rather than a slot-keyed one,
                            // because a row's statement id has to name a value, not occupy a position.
                            StatementId::Other(self.set.statement_fallbacks.intern_text(cell))
                        }
                    }
                });

        let row_index = self.set.compound_ids.len();
        self.set.compound_ids.push(compound_id);
        self.set.taxon_ids.push(taxon_id);
        // The reference node is a `prov:wasDerivedFrom` URI, not a QID, so it
        // cannot go in `references` -- that is a `QidDictionary` of u32s. A
        // `SparseStrings` keyed by row keeps the 64-hex hash interned instead of
        // copied per row, the same way `statement_fallbacks` does.
        if let Some(node) = crate::non_empty(row.reference_node) {
            self.set.reference_node_hashes.set(row_index, Some(node));
        }
        self.set.reference_ids.push(reference_id);
        if statement != StatementId::Absent {
            self.set.statements.resize(row_index, StatementId::Absent);
            self.set.statements.push(statement);
        }
    }

    /// Populate dictionary metadata for already-interned entities without
    /// appending an occurrence row, returning whether any entity on `row`
    /// matched this builder's dictionaries.
    ///
    /// Entities absent from the occurrence dictionaries are ignored so a broad
    /// dictionary stream never inflates `compounds` or `references` with unused
    /// slots.
    pub fn enrich(&mut self, row: RawRow<'_>) -> bool {
        let compound_slot = lookup_slot(&self.set.compounds, row.compound_qid);
        if let Some(slot) = compound_slot {
            self.populate_compound(slot, row);
        }
        let taxon_slot = lookup_slot(&self.set.taxa, row.taxon_qid);
        if let Some(slot) = taxon_slot {
            self.set
                .taxon_names
                .set(slot, crate::non_empty(row.taxon_name));
        }
        let reference_slot = lookup_slot(&self.set.references, row.reference_qid);
        if let Some(slot) = reference_slot {
            self.populate_reference(slot, row);
        }
        compound_slot.is_some() || taxon_slot.is_some() || reference_slot.is_some()
    }

    /// Intern and populate compound or reference dictionary metadata without
    /// appending an occurrence row, so extra entities arriving in an
    /// unconstrained dictionary stream can be retained for seeding subsequent
    /// searches.
    pub fn intern_dictionary_row(&mut self, row: RawRow<'_>) {
        if let Some(id) = intern_qid(&mut self.set.compounds, row.compound_qid) {
            self.grow_compounds();
            self.populate_compound(id as usize, row);
        }
        if let Some(id) = intern_qid(&mut self.set.references, row.reference_qid) {
            self.grow_references();
            self.populate_reference(id as usize, row);
        }
    }

    pub(super) fn populate_compound(&mut self, compound_slot: usize, row: RawRow<'_>) {
        if let Some(name) = crate::non_empty(row.name) {
            self.set.compound_names.set(compound_slot, Some(name));
        }
        if let Some(key) = non_empty(row.inchikey) {
            self.set.compound_inchikeys.set(compound_slot, Some(key));
        }
        if let Some(smiles) = non_empty(row.smiles) {
            self.set.compound_smiles.set(compound_slot, Some(smiles));
        }
        // First write wins, like every other compound column: see
        // `SparseStrings::set` for why, and for what it costs to do otherwise.
        if let Some(mass) = row.mass.filter(|m| m.is_finite())
            && let Some(cell) = self.set.compound_masses.get_mut(compound_slot)
            && cell.is_nan()
        {
            *cell = mass;
        }
        if self.set.compound_formulas.get(compound_slot).is_none()
            && let Some(formula) = non_empty(row.formula)
        {
            if formula.is_ascii() {
                self.set.compound_formulas.set(compound_slot, Some(formula));
            } else {
                let normalized = crate::normalize_formula_subscripts(formula);
                self.set
                    .compound_formulas
                    .set(compound_slot, Some(&normalized));
            }
        }
    }

    pub(super) fn populate_reference(&mut self, slot: usize, row: RawRow<'_>) {
        if let Some(title) = non_empty(row.ref_title) {
            self.set.reference_titles.set(slot, Some(title));
        }
        // Normalised here rather than at parse time, so the `doi.org`
        // prefix is stripped and the case fixed once per *reference* rather
        // than once per row mentioning it.
        if self.set.reference_dois.get(slot).is_none()
            && let Some(doi) = non_empty(row.ref_doi).and_then(crate::normalize_doi)
        {
            self.set.reference_dois.set(slot, Some(&doi));
        }
        if let Some(year) = row.pub_year
            && let Some(cell) = self.set.reference_years.get_mut(slot)
            && cell.is_none()
        {
            *cell = Some(year);
        }
    }

    /// Make sure the compound columns cover every interned compound.
    ///
    /// Ids are minted in first-seen order, so `compounds.len()` is at most one
    /// past the end and the loop runs at most once. It is a loop so that a
    /// caller which interleaves builders cannot leave the columns ragged. A
    /// counted loop rather than `while`, so that a wrong slot count leaves the
    /// columns the wrong length instead of never ending.
    fn grow_compounds(&mut self) {
        let first = self.set.compound_masses.len();
        for next in first..self.set.compounds.len() {
            self.set.compound_names.set(next, None);
            self.set.compound_inchikeys.set(next, None);
            self.set.compound_smiles.set(next, None);
            self.set.compound_formulas.set(next, None);
            self.set.compound_masses.push(NO_MASS);
        }
    }

    /// Make sure the reference columns cover every interned reference.
    fn grow_references(&mut self) {
        let first = self.set.reference_years.len();
        for next in first..self.set.references.len() {
            self.set.reference_titles.set(next, None);
            self.set.reference_dois.set(next, None);
            self.set.reference_years.push(None);
        }
    }

    /// Take the statistics and produce the set.
    #[must_use]
    pub fn build(self) -> ColumnarResultSet {
        let compounds = self.set.compounds.used(self.set.compound_ids.iter());
        let taxa = self.set.taxa.used(self.set.taxon_ids.iter());
        let references = self.set.references.used(self.set.reference_ids.iter());
        let unique = self.distinct_triples();
        let mut set = self.set;
        set.generation = next_generation();
        set.stats = DatasetStats {
            n_compounds: compounds.len(),
            n_taxa: taxa.len(),
            n_references: references.len(),
            n_entries: set.compound_ids.len(),
            n_entries_unique: unique,
        };
        set
    }

    /// The number of distinct compound-taxon-reference triples.
    ///
    /// This is the endpoint's `COUNT(DISTINCT CONCAT(…))`, the one number a local
    /// count used to be unable to reproduce: the old parser deduplicated
    /// *before* the rows existed, so it could only report a lower bound. Sorting
    /// a copy of the id columns is the price of being exact, and the copy is
    /// dropped before [`build`](Self::build) returns.
    fn distinct_triples(&self) -> usize {
        let mut seen =
            FxHashSet::with_capacity_and_hasher(self.set.compound_ids.len(), FxBuildHasher);
        for ((&c, &t), &r) in self
            .set
            .compound_ids
            .iter()
            .zip(&self.set.taxon_ids)
            .zip(&self.set.reference_ids)
        {
            seen.insert((c, t, r));
        }
        seen.len()
    }
}

/// A trimmed, non-empty optional string.
pub(super) fn non_empty(value: Option<&str>) -> Option<&str> {
    value.and_then(crate::non_empty)
}
