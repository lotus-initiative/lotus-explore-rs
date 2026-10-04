// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]

use crate::features::curation::repositories::CurationKnowledgeRepository;
use lotus_curation::CurationError;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AskCacheKey {
    Taxon {
        compound_qid: String,
        taxon_qid: String,
    },
    TaxonWithRef {
        compound_qid: String,
        taxon_qid: String,
        ref_qid: String,
    },
}

/// What a whole run already knows, so the per-row path asks nothing.
///
/// Three caches, and the reason they are here rather than scattered through the
/// row loop is arithmetic: a curation run is over *N* rows, and the questions are
/// per row, so anything not batched costs `N` requests to a shared public
/// endpoint. A 200-row import used to be up to 800. It is now five, whatever
/// `N` is: one for every compound's `InChIKey`, one for taxon names, one for
/// DOIs, one for "does this occurrence exist" and one for "...by this paper".
#[derive(Default)]
pub struct OccurrenceAskCache {
    values: HashMap<AskCacheKey, bool>,
}

fn read_cached_ask(cache: &Mutex<OccurrenceAskCache>, key: &AskCacheKey) -> Option<bool> {
    cache
        .lock()
        .ok()
        .and_then(|guard| guard.values.get(key).copied())
}

fn write_cached_ask(cache: &Mutex<OccurrenceAskCache>, key: AskCacheKey, value: bool) {
    if let Ok(mut guard) = cache.lock() {
        guard.values.insert(key, value);
    }
}

/// Record a batch of `(compound, taxon)` answers, all the same verdict.
///
/// The shape the prefetch has: it asks "which of these pairs are recorded" and
/// gets back the ones that are, so the misses have to be written as `false` as
/// well. A cache that only stored hits would send the row loop to the network for
/// every pair that is *not* already there, which is most of them.
pub fn record_pairs_cached(
    cache: &Mutex<OccurrenceAskCache>,
    pairs: impl IntoIterator<Item = (String, String)>,
    recorded: bool,
) {
    if let Ok(mut guard) = cache.lock() {
        for (compound_qid, taxon_qid) in pairs {
            guard.values.insert(
                AskCacheKey::Taxon {
                    compound_qid,
                    taxon_qid,
                },
                recorded,
            );
        }
    }
}

/// The same for the three-part question.
pub fn record_triples_cached(
    cache: &Mutex<OccurrenceAskCache>,
    triples: impl IntoIterator<Item = (String, String, String)>,
    recorded: bool,
) {
    if let Ok(mut guard) = cache.lock() {
        for (compound_qid, taxon_qid, ref_qid) in triples {
            guard.values.insert(
                AskCacheKey::TaxonWithRef {
                    compound_qid,
                    taxon_qid,
                    ref_qid,
                },
                recorded,
            );
        }
    }
}

/// How many answers are already known, for the tests and the log line.
pub fn cached_answer_count(cache: &Mutex<OccurrenceAskCache>) -> usize {
    cache.lock().map_or(0, |guard| guard.values.len())
}

pub async fn compound_has_taxon_cached(
    repository: &dyn CurationKnowledgeRepository,
    cache: &Mutex<OccurrenceAskCache>,
    compound_qid: &str,
    taxon_qid: &str,
) -> Result<bool, CurationError> {
    let key = AskCacheKey::Taxon {
        compound_qid: compound_qid.into(),
        taxon_qid: taxon_qid.into(),
    };

    if let Some(cached) = read_cached_ask(cache, &key) {
        return Ok(cached);
    }

    let value = repository
        .compound_has_taxon(compound_qid, taxon_qid)
        .await?;
    write_cached_ask(cache, key, value);
    Ok(value)
}

pub async fn compound_has_taxon_with_ref_cached(
    repository: &dyn CurationKnowledgeRepository,
    cache: &Mutex<OccurrenceAskCache>,
    compound_qid: &str,
    taxon_qid: &str,
    ref_qid: &str,
) -> Result<bool, CurationError> {
    let key = AskCacheKey::TaxonWithRef {
        compound_qid: compound_qid.into(),
        taxon_qid: taxon_qid.into(),
        ref_qid: ref_qid.into(),
    };

    if let Some(cached) = read_cached_ask(cache, &key) {
        return Ok(cached);
    }

    let value = repository
        .compound_has_taxon_with_ref(compound_qid, taxon_qid, ref_qid)
        .await?;
    write_cached_ask(cache, key, value);
    Ok(value)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use crate::features::curation::repositories::{BoxedFuture, ResolveTaxonResult};
    use futures::executor::block_on;
    use lotus_curation::WikidataCompound;
    use std::collections::HashSet;

    #[derive(Default)]
    struct MockRepo {
        ask_calls: Mutex<usize>,
    }

    impl CurationKnowledgeRepository for MockRepo {
        fn fetch_compound_by_inchikey(
            &self,
            _inchikey: &str,
        ) -> BoxedFuture<'_, Result<Option<WikidataCompound>, CurationError>> {
            Box::pin(async { Ok(None) })
        }

        fn resolve_or_create_taxon(
            &self,
            _name: &str,
            _pre_resolved_qid: Option<&str>,
        ) -> BoxedFuture<'_, ResolveTaxonResult> {
            Box::pin(async { Ok((None, Vec::new())) })
        }

        fn resolve_reference_qid(
            &self,
            _doi: &str,
        ) -> BoxedFuture<'_, Result<Option<String>, CurationError>> {
            Box::pin(async { Ok(None) })
        }

        fn compound_has_taxon_with_ref(
            &self,
            _compound_qid: &str,
            _taxon_qid: &str,
            _ref_qid: &str,
        ) -> BoxedFuture<'_, Result<bool, CurationError>> {
            Box::pin(async { Ok(false) })
        }

        fn compound_has_taxon(
            &self,
            _compound_qid: &str,
            _taxon_qid: &str,
        ) -> BoxedFuture<'_, Result<bool, CurationError>> {
            Box::pin(async {
                if let Ok(mut calls) = self.ask_calls.lock() {
                    *calls += 1;
                }
                Ok(false)
            })
        }

        // Two trait items, so two bodies are the trait's shape rather than a copy
        // someone made. `resolve_taxon_qids_batch` cannot call
        // `resolve_reference_qids_batch`: they are separate items of the same
        // trait, and neither is a specialisation of the other. A helper both call
        // would be three functions to express two empty stubs.
        //
        // The marker goes last because it has to be the line immediately above the
        // function; with prose in between it is not seen and the pair is reported
        // anyway, which is a gate that looks broken rather than one that is.
        // dejadoc: allow
        fn resolve_taxon_qids_batch(
            &self,
            _names: &[String],
        ) -> BoxedFuture<'_, Result<HashMap<String, String>, CurationError>> {
            Box::pin(async { Ok(HashMap::new()) })
        }

        // The other half of the pair above, allowed for the same reason.
        // dejadoc: allow
        fn resolve_reference_qids_batch(
            &self,
            _dois: &[String],
        ) -> BoxedFuture<'_, Result<HashMap<String, String>, CurationError>> {
            Box::pin(async { Ok(HashMap::new()) })
        }

        fn fetch_compounds_by_inchikeys(
            &self,
            _keys: &[String],
        ) -> BoxedFuture<'_, Result<HashMap<String, WikidataCompound>, CurationError>> {
            Box::pin(async { Ok(HashMap::new()) })
        }

        fn existing_occurrences(
            &self,
            _pairs: &[(String, String)],
        ) -> BoxedFuture<'_, Result<HashSet<(String, String)>, CurationError>> {
            Box::pin(async { Ok(HashSet::new()) })
        }

        fn existing_occurrences_with_ref(
            &self,
            _triples: &[(String, String, String)],
        ) -> BoxedFuture<'_, Result<HashSet<(String, String, String)>, CurationError>> {
            Box::pin(async { Ok(HashSet::new()) })
        }
    }

    #[test]
    fn ask_cache_reuses_result_for_same_compound_taxon_pair() {
        let repo = MockRepo::default();
        let cache = Mutex::new(OccurrenceAskCache::default());

        let first = block_on(compound_has_taxon_cached(&repo, &cache, "Q1", "Q2"))
            .expect("first ask result");
        let second = block_on(compound_has_taxon_cached(&repo, &cache, "Q1", "Q2"))
            .expect("second ask result");

        assert!(!first);
        assert!(!second);
        assert_eq!(*repo.ask_calls.lock().expect("counter lock"), 1);
    }
}
