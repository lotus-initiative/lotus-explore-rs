// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Row-level and dictionary-level accessors for [`ColumnarResultSet`].

use std::borrow::Cow;
use std::sync::Arc;

use super::qid_dict::slot_to_id;
use super::{ColumnarResultSet, CompoundEntry, DatasetStats, NO_VALUE, qid_text};

impl ColumnarResultSet {
    /// How many rows.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.compound_ids.len()
    }

    /// Whether the set has no rows.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.compound_ids.is_empty()
    }

    /// The counts, computed once when the set was built.
    ///
    /// Exact over the whole set, because a columnar set holds every row the
    /// endpoint returned and deduplicates nothing.
    #[must_use]
    pub fn stats(&self) -> DatasetStats {
        self.stats.clone()
    }

    /// How many *distinct* statements are kept as text, not `{QID}-{UUID}`.
    ///
    /// Expected to be zero, and worth measuring rather than assuming: the
    /// round-tripped text is identical either way, so a test that only checks the
    /// text cannot tell a sixteen-byte statement from a whole string of one.
    #[must_use]
    pub const fn statement_fallback_entries(&self) -> usize {
        self.statement_fallbacks.distinct_len()
    }

    /// Every distinct compound QID the set holds, in first-seen order.
    ///
    /// Already deduplicated, which makes this the cheap way to hash a result: the
    /// row-at-a-time path collected a `Vec<&str>` of every row's compound QID and
    /// sorted it to get here, allocating three million entries to find two and a
    /// half million.
    pub fn compound_qids(&self) -> impl Iterator<Item = u32> + '_ {
        self.compounds.numeric_ids()
    }

    /// The compound's numeric QID for row `row`.
    ///
    /// The number, not the text: the text is a `Q` and those digits, and a caller
    /// that wants it calls [`qid_text`]. Every row shares a handful of QIDs, so
    /// handing back the number keeps a filter or a sort comparing four bytes
    /// instead of a string.
    #[must_use]
    pub fn compound_qid(&self, row: usize) -> Option<u32> {
        self.compounds.get(*self.compound_ids.get(row)?)
    }

    /// The taxon of row `row`, as a numeric QID, if it has one.
    #[must_use]
    pub fn taxon_qid(&self, row: usize) -> Option<u32> {
        self.taxa.get(*self.taxon_ids.get(row)?)
    }

    /// The reference of row `row`, as a numeric QID, if it has one.
    #[must_use]
    pub fn reference_qid(&self, row: usize) -> Option<u32> {
        self.references.get(*self.reference_ids.get(row)?)
    }

    /// The compound's QID as text, for a link or a displayed cell.
    #[must_use]
    pub fn compound_qid_text(&self, row: usize) -> Option<String> {
        self.compound_qid(row).map(qid_text)
    }

    /// The taxon of row `row` as text.
    #[must_use]
    pub fn taxon_qid_text(&self, row: usize) -> Option<String> {
        self.taxon_qid(row).map(qid_text)
    }

    /// The reference of row `row` as text.
    #[must_use]
    pub fn reference_qid_text(&self, row: usize) -> Option<String> {
        self.reference_qid(row).map(qid_text)
    }

    /// The reference node's QID for `row`, if the occurrence names one.
    #[must_use]
    pub fn reference_node_text(&self, row: usize) -> Option<String> {
        self.reference_node_hashes.get(row).map(str::to_string)
    }

    /// The compound's label, falling back to its QID.
    ///
    /// The fallback is the one the table applies to a row with no label, kept
    /// here so a caller reading the set and a caller rebuilding a [`CompoundEntry`]
    /// agree on what the cell says.
    #[must_use]
    pub fn compound_label(&self, row: usize) -> Option<&str> {
        let id = *self.compound_ids.get(row)?;
        self.compound_names
            .get(id as usize)
            .filter(|n| !n.is_empty())
    }

    /// The label if the compound has one, and its QID if it does not.
    ///
    /// Borrowed where there is a label and allocated where there is not, which is
    /// what the cell has always shown: a nameless compound is displayed as its
    /// identifier rather than as nothing.
    #[must_use]
    pub fn compound_label_or_qid(&self, row: usize) -> Option<Cow<'_, str>> {
        let id = *self.compound_ids.get(row)?;
        self.compound_names
            .get(id as usize)
            .filter(|n| !n.is_empty())
            .map_or_else(
                || self.compounds.get(id).map(|n| Cow::Owned(qid_text(n))),
                |label| Some(Cow::Borrowed(label)),
            )
    }

    /// The compound's `InChIKey`, if it has one.
    ///
    /// The row-level readers below exist so that sorting and filtering never have
    /// to build a [`CompoundEntry`]. Materialising a row to compare two of them
    /// would allocate thirteen `Arc<str>`s per comparison, and a sort of three
    /// million rows does three million of those.
    #[must_use]
    pub fn inchikey(&self, row: usize) -> Option<&str> {
        self.compound_inchikeys
            .get(Self::slot(self.compound_ids.get(row))?)
    }

    /// The compound's SMILES, if it has one.
    #[must_use]
    pub fn smiles(&self, row: usize) -> Option<&str> {
        self.compound_smiles
            .get(Self::slot(self.compound_ids.get(row))?)
    }

    /// The compound's molecular formula, if it has one.
    #[must_use]
    pub fn formula(&self, row: usize) -> Option<&str> {
        self.compound_formulas
            .get(Self::slot(self.compound_ids.get(row))?)
    }

    /// The reference's title, if it has one.
    #[must_use]
    pub fn reference_title(&self, row: usize) -> Option<&str> {
        self.reference_titles
            .get(Self::slot(self.reference_ids.get(row))?)
    }

    /// The reference's DOI, if it has one.
    #[must_use]
    pub fn reference_doi(&self, row: usize) -> Option<&str> {
        self.reference_dois
            .get(Self::slot(self.reference_ids.get(row))?)
    }

    /// The reference's publication year, if it has one.
    #[must_use]
    pub fn pub_year(&self, row: usize) -> Option<i16> {
        self.reference_year(slot_to_id(Self::slot(self.reference_ids.get(row))?))
    }

    /// The compound's mass in daltons, if it has one.
    #[must_use]
    pub fn mass(&self, row: usize) -> Option<f64> {
        self.compound_mass(slot_to_id(Self::slot(self.compound_ids.get(row))?))
    }

    /// The dictionary slot `id` names, or `None` when the row does not have one.
    ///
    /// A row with no taxon or no reference carries [`NO_VALUE`], which casts to
    /// `u32::MAX`; returning `None` for it keeps every caller from having to
    /// remember that, and from a slot index of four billion reaching a `Vec`.
    fn slot(id: Option<&u32>) -> Option<usize> {
        let id = *id?;
        (id != NO_VALUE).then_some(id as usize)
    }

    /// The scientific name of row `row`'s taxon.
    #[must_use]
    pub fn taxon_label(&self, row: usize) -> Option<&str> {
        self.taxon_names.get(Self::slot(self.taxon_ids.get(row))?)
    }

    /// The statement backing row `row`, as the table shows it.
    #[must_use]
    pub fn statement_text(&self, row: usize) -> Option<String> {
        let statement = *self.statements.get(row)?;
        let compound = self.compound_qid(row)?;
        statement.text(&qid_text(compound), &self.statement_fallbacks)
    }

    /// The compound's mass, or `None` when it has none.
    #[must_use]
    pub fn compound_mass(&self, compound_id: u32) -> Option<f64> {
        self.compound_masses
            .get(compound_id as usize)
            .copied()
            .filter(|m| !m.is_nan())
    }

    /// The reference's publication year, or `None` when it has none.
    #[must_use]
    pub(super) fn reference_year(&self, reference_id: u32) -> Option<i16> {
        self.reference_years
            .get(reference_id as usize)
            .copied()
            .flatten()
    }

    /// Rebuild row `row` as a [`CompoundEntry`].
    ///
    /// For the paths that still want one row at a time: the export formats, and
    /// any caller that has to hand a single result to something expecting the
    /// old shape. Interned strings are cloned by [`Arc`] reference count rather
    /// than re-allocated per row.
    #[must_use]
    pub fn entry(&self, row: usize) -> Option<CompoundEntry> {
        let compound_id = *self.compound_ids.get(row)?;
        let reference_id = *self.reference_ids.get(row)?;
        let compound_slot = compound_id as usize;
        let taxon_slot = Self::slot(self.taxon_ids.get(row));
        let reference_slot = Self::slot(self.reference_ids.get(row));
        let empty = || Arc::<str>::from("");
        let owned = |text: Option<String>| text.map_or_else(empty, Arc::<str>::from);
        Some(CompoundEntry {
            compound_qid: Arc::<str>::from(self.compound_qid_text(row)?),
            name: self
                .compound_names
                .get_arc(compound_slot)
                .unwrap_or_else(empty),
            // The three QID cells keep the `Arc<str>`-and-empty convention rather
            // than the `Option` one, because `n_taxa` and `n_references` are
            // defined in terms of what is empty.
            inchikey: self.compound_inchikeys.get_arc(compound_slot),
            smiles: self.compound_smiles.get_arc(compound_slot),
            mass: self.compound_mass(compound_id),
            formula: self.compound_formulas.get_arc(compound_slot),
            taxon_qid: owned(self.taxon_qid_text(row)),
            taxon_name: taxon_slot
                .and_then(|slot| self.taxon_names.get_arc(slot))
                .unwrap_or_else(empty),
            reference_qid: owned(self.reference_qid_text(row)),
            reference_node: self
                .reference_node_hashes
                .get_arc(row)
                .unwrap_or_else(empty),
            ref_title: reference_slot.and_then(|slot| self.reference_titles.get_arc(slot)),
            ref_doi: reference_slot.and_then(|slot| self.reference_dois.get_arc(slot)),
            pub_year: self.reference_year(reference_id),
            statement: self.statement_text(row).map(Arc::<str>::from),
        })
    }
}
