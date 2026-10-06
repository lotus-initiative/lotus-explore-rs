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
/// Here rather than scattered through the row loop because of arithmetic: *N*
/// rows times a per-row question is `N` requests to a shared public endpoint, and
/// a 200-row import was up to 800. It is now five whatever `N` is -- one each for
/// compound `InChIKey`s, taxon names, DOIs, "does this occurrence exist" and
/// "...by this paper".
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
/// The prefetch asks "which of these pairs are recorded" and gets only the ones
/// that are, so misses are written as `false` too: a cache of hits only would send
/// the row loop to the network for most pairs.
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

/// How many answers are already known.
///
/// Test-only: it exists so a test can assert the cache was filled, and nothing in
/// the pipeline reads it. Without the gate it is dead code in every non-test
/// build, which `-D dead-code` correctly refuses.
#[cfg(test)]
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
#[path = "occurrence_cache/tests.rs"]
mod tests;
