// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Deterministic `InChIKey` deduplication for SMILES records.

use std::collections::BTreeMap;

use super::{SmilesKind, SmilesRecord};

/// Selects one record per `InChIKey`, preferring P2017 isomeric SMILES over P233.
///
/// Multiple values of the preferred property are resolved deterministically by
/// source SMILES then Wikidata ID, so repeated pulls yield the same archive.
#[must_use]
pub fn deduplicate_by_inchi_key(records: Vec<SmilesRecord>) -> Vec<SmilesRecord> {
    let mut selected = BTreeMap::new();
    for record in records {
        match selected.entry(record.inchi_key.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(record);
            }
            std::collections::btree_map::Entry::Occupied(mut entry)
                if preferred_over(&record, entry.get()) =>
            {
                entry.insert(record);
            }
            std::collections::btree_map::Entry::Occupied(_) => {}
        }
    }
    selected.into_values().collect()
}

fn preferred_over(candidate: &SmilesRecord, current: &SmilesRecord) -> bool {
    let candidate_priority = matches!(candidate.smiles_kind, SmilesKind::Isomeric);
    let current_priority = matches!(current.smiles_kind, SmilesKind::Isomeric);
    if candidate_priority != current_priority {
        return candidate_priority;
    }
    let candidate_key = (candidate.smiles.as_str(), candidate.wikidata_id.as_str());
    let current_key = (current.smiles.as_str(), current.wikidata_id.as_str());
    candidate_key < current_key
}
