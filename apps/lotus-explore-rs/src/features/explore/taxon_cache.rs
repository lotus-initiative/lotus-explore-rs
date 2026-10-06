// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! In-process taxon name → resolution cache.
//!
//! The cached value is the whole resolution, not just the QID. Caching only the
//! QID makes the notice depend on the cache: the first run reports an ambiguous
//! name, the second — never re-reading the candidate list — cannot, so one query
//! tells the user two different things.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::features::explore::types::LookupNotice;
use lotus_model::TaxonNameSource;

/// A resolved taxon name, together with the candidates that made it ambiguous.
///
/// `candidates` is empty when exactly one candidate matched; a non-empty list
/// is what [`LookupNotice::AmbiguousTaxon`] is reconstructed from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedTaxon {
    /// The chosen Wikidata QID.
    pub qid: String,
    /// The chosen candidate's display name, as the ambiguity notice shows it.
    pub label: String,
    /// Which of P225 and P1843 the name came from, which is what earns the
    /// common-name notice.
    pub source: TaxonNameSource,
    /// `"Name (QID)"` strings for the candidates offered, empty when unambiguous.
    pub candidates: Vec<String>,
}

impl CachedTaxon {
    /// The notices this resolution earns, in the order they apply.
    ///
    /// Same answer on every run, because the candidate list and the property it
    /// came from travel with the QID through the cache rather than being
    /// re-derived from a lookup that may not happen again.
    #[must_use]
    pub fn warnings(&self) -> Vec<LookupNotice> {
        let mut warnings = Vec::new();
        if self.source == TaxonNameSource::Common {
            warnings.push(LookupNotice::CommonName {
                chosen_name: self.label.clone(),
                chosen_qid: self.qid.clone(),
            });
        }
        if !self.candidates.is_empty() {
            warnings.push(LookupNotice::AmbiguousTaxon {
                chosen_name: self.label.clone(),
                chosen_qid: self.qid.clone(),
                candidates: self.candidates.clone(),
            });
        }
        warnings
    }
}

type TaxonCache = HashMap<String, CachedTaxon>;

thread_local! {
    static CACHE: RefCell<TaxonCache> = RefCell::new(HashMap::new());
}

/// Returns the cached resolution for the given taxon `name`, or `None` if not cached.
pub fn lookup(name: &str) -> Option<CachedTaxon> {
    let key = name.trim().to_lowercase();
    if key.is_empty() {
        return None;
    }
    CACHE.with(|cache| cache.borrow().get(&key).cloned())
}

/// Stores `resolution` in the cache under the normalised form of `name`.
pub fn store(name: &str, resolution: &CachedTaxon) {
    let key = name.trim().to_lowercase();
    if key.is_empty() || resolution.qid.trim().is_empty() {
        return;
    }
    CACHE.with(|cache| {
        cache.borrow_mut().insert(key, resolution.clone());
    });
}

#[cfg(test)]
#[path = "taxon_cache/tests.rs"]
mod tests;
