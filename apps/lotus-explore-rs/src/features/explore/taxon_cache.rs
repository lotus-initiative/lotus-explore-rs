// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! In-process taxon name → resolution cache.
//!
//! The cached value is the whole resolution, not just the QID. Caching only the
//! QID makes the notice depend on whether the lookup hit the cache: the first
//! run reports an ambiguous name, and the second run — which never re-reads the
//! candidate list — cannot, so the same query tells the user two different
//! things. A cache that holds only the answer to the question it was asked is
//! not enough when the answer has a reason attached.

use std::cell::RefCell;
use std::collections::HashMap;

use crate::features::explore::types::LookupNotice;
use lotus_model::TaxonNameSource;

/// A resolved taxon name, together with the candidates that made it ambiguous.
///
/// `candidates` is empty when exactly one candidate matched; a non-empty list
/// is what [`LookupNotice::Ambiguous`] is reconstructed from.
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
            warnings.push(LookupNotice::Ambiguous {
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
mod tests {
    use super::*;

    #[test]
    fn lookup_returns_none_for_empty_key() {
        assert!(lookup("").is_none());
        assert!(lookup("   ").is_none());
    }

    fn resolved(qid: &str, label: &str) -> CachedTaxon {
        CachedTaxon {
            qid: qid.to_owned(),
            label: label.to_owned(),
            source: TaxonNameSource::Scientific,
            candidates: Vec::new(),
        }
    }

    #[test]
    fn store_and_lookup_roundtrip() {
        store("Gentiana lutea", &resolved("Q2598745", "Gentiana lutea"));
        let result = lookup("gentiana lutea");
        assert_eq!(result.map(|cached| cached.qid), Some("Q2598745".into()));
    }

    #[test]
    fn store_ignores_empty_qid() {
        store("Somespecies", &resolved("", "Somespecies"));
        assert!(lookup("somespecies").is_none());
    }

    #[test]
    fn an_unambiguous_resolution_earns_no_notice() {
        assert_eq!(resolved("Q1", "Rosa").warnings().len(), 0);
    }

    #[test]
    fn a_common_name_resolution_earns_the_common_name_notice_on_every_read() {
        let cached = CachedTaxon {
            qid: "Q777".to_owned(),
            label: "Gentian".to_owned(),
            source: TaxonNameSource::Common,
            candidates: Vec::new(),
        };
        store("gentian", &cached);

        let warnings = lookup("Gentian").map(|hit| hit.warnings());
        assert!(
            matches!(warnings, Some(ref w) if matches!(w[..], [LookupNotice::CommonName { .. }])),
            "expected the common-name notice, got {warnings:?}"
        );
    }

    #[test]
    fn a_scientific_resolution_earns_nothing() {
        assert_eq!(resolved("Q1", "Gentiana lutea").warnings().len(), 0);
    }

    #[test]
    fn a_common_name_that_is_also_ambiguous_earns_both_notices() {
        let cached = CachedTaxon {
            qid: "Q777".to_owned(),
            label: "Gentian".to_owned(),
            source: TaxonNameSource::Common,
            candidates: vec!["Gentian (Q777)".to_owned(), "Gentian (Q778)".to_owned()],
        };
        assert!(matches!(
            cached.warnings().as_slice(),
            [
                LookupNotice::CommonName { .. },
                LookupNotice::Ambiguous { .. }
            ]
        ));
    }

    #[test]
    fn an_ambiguous_resolution_reproduces_the_same_notice_on_every_read() {
        let candidates = vec![
            "Bacteria (Q10876)".to_owned(),
            "Bacteria (Q4034791)".to_owned(),
        ];
        store(
            "bacteria",
            &CachedTaxon {
                qid: "Q10876".to_owned(),
                label: "Bacteria".to_owned(),
                source: TaxonNameSource::Scientific,
                candidates,
            },
        );

        let first = lookup("bacteria").map(|cached| cached.warnings());
        let second = lookup("Bacteria").map(|cached| cached.warnings());

        assert_eq!(
            first, second,
            "the notices must not depend on the cache path"
        );
        assert!(matches!(
            first.as_deref(),
            Some([LookupNotice::Ambiguous { .. }])
        ));
    }
}
