// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Dictionary pre-seeding from a prior [`ColumnarResultSet`] into a [`ColumnarBuilder`].

use super::bitmask::Bitmask;
use super::builder::ColumnarBuilder;
use super::qid_dict::{QidDictionary, slot_to_id};
use super::{ColumnarResultSet, RawRow};

impl ColumnarBuilder {
    /// Pre-populate compound and reference metadata for any entity already
    /// present in `previous`, so dictionary enrichment only queries the
    /// endpoint for entities not yet seen.
    pub fn seed_from(&mut self, previous: &ColumnarResultSet) {
        self.seeded_compounds.resize(self.set.compounds.len());
        for slot in 0..self.set.compounds.len() {
            let slot_id = slot_to_id(slot);
            if let Some(prev_id) = self
                .set
                .compounds
                .get(slot_id)
                .and_then(|numeric| previous.compounds.lookup(numeric))
            {
                let prev_slot = prev_id as usize;
                self.populate_compound(
                    slot,
                    RawRow {
                        name: previous.compound_names.get(prev_slot).unwrap_or_default(),
                        inchikey: previous.compound_inchikeys.get(prev_slot),
                        smiles: previous.compound_smiles.get(prev_slot),
                        mass: previous.compound_mass(prev_id),
                        formula: previous.compound_formulas.get(prev_slot),
                        ..RawRow::default()
                    },
                );
                self.seeded_compounds.insert(slot_id);
            }
        }

        self.seeded_references.resize(self.set.references.len());
        for slot in 0..self.set.references.len() {
            let slot_id = slot_to_id(slot);
            if let Some(prev_id) = self
                .set
                .references
                .get(slot_id)
                .and_then(|numeric| previous.references.lookup(numeric))
            {
                let prev_slot = prev_id as usize;
                self.populate_reference(
                    slot,
                    RawRow {
                        ref_title: previous.reference_titles.get(prev_slot),
                        ref_doi: previous.reference_dois.get(prev_slot),
                        pub_year: previous.reference_year(prev_id),
                        ..RawRow::default()
                    },
                );
                self.seeded_references.insert(slot_id);
            }
        }
    }
}

/// Every distinct QID in `dict` whose slot is not yet marked in `seeded`, in
/// first-seen order.
pub(super) fn unseeded_qids<'a>(
    dict: &'a QidDictionary,
    seeded: &'a Bitmask,
) -> impl Iterator<Item = u32> + 'a {
    dict.numeric_ids()
        .enumerate()
        .filter(|&(slot, _)| !seeded.contains(slot_to_id(slot)))
        .map(|(_, qid)| qid)
}
