// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `prefetch`, in their own file.

#![allow(clippy::expect_used)]

use super::*;
use crate::features::curation::repositories::{
    BoxedFuture, OccurrencePair, OccurrenceTriple, ResolveTaxonResult,
};
use futures::executor::block_on;
use lotus_curation::WikidataCompound;
use std::sync::Mutex;

/// A repository that counts what it was asked, and answers everything.
///
/// The counter is the whole point: the claim being tested is about *how many
/// requests* a run makes, and a mock that answers correctly cannot show it.
#[derive(Default)]
struct Counting {
    calls: Mutex<Vec<&'static str>>,
    /// Which `InChIKey`s exist, when the test cares. `None` means "all of
    /// them", which is what the query-count tests want; a `Some` is how the
    /// "this compound is not in Wikidata" path gets exercised.
    known: Option<HashSet<String>>,
}

impl Counting {
    fn knowing(keys: &[&str]) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            known: Some(keys.iter().map(|k| (*k).to_string()).collect()),
        }
    }
}

impl Counting {
    fn count(&self, what: &'static str) {
        self.calls.lock().expect("counter").push(what);
    }

    fn total(&self) -> usize {
        self.calls.lock().expect("counter").len()
    }
}

impl CurationKnowledgeRepository for Counting {
    fn fetch_compound_by_inchikey(
        &self,
        _inchikey: &str,
    ) -> BoxedFuture<'_, Result<Option<WikidataCompound>, CurationError>> {
        self.count("compound-single");
        Box::pin(async { Ok(None) })
    }

    fn resolve_or_create_taxon(
        &self,
        _name: &str,
        _pre_resolved_qid: Option<&str>,
    ) -> BoxedFuture<'_, ResolveTaxonResult> {
        self.count("taxon-single");
        Box::pin(async { Ok((None, Vec::new())) })
    }

    fn resolve_reference_qid(
        &self,
        _doi: &str,
    ) -> BoxedFuture<'_, Result<Option<String>, CurationError>> {
        self.count("reference-single");
        Box::pin(async { Ok(None) })
    }

    fn compound_has_taxon_with_ref(
        &self,
        _compound_qid: &str,
        _taxon_qid: &str,
        _ref_qid: &str,
    ) -> BoxedFuture<'_, Result<bool, CurationError>> {
        self.count("occurrence-single");
        Box::pin(async { Ok(false) })
    }

    fn compound_has_taxon(
        &self,
        _compound_qid: &str,
        _taxon_qid: &str,
    ) -> BoxedFuture<'_, Result<bool, CurationError>> {
        self.count("occurrence-single");
        Box::pin(async { Ok(false) })
    }

    fn resolve_taxon_qids_batch(
        &self,
        _names: &[String],
    ) -> BoxedFuture<'_, Result<HashMap<String, String>, CurationError>> {
        self.count("taxon-batch");
        Box::pin(async { Ok(HashMap::new()) })
    }

    fn resolve_reference_qids_batch(
        &self,
        _dois: &[String],
    ) -> BoxedFuture<'_, Result<HashMap<String, String>, CurationError>> {
        self.count("reference-batch");
        Box::pin(async { Ok(HashMap::new()) })
    }

    fn fetch_compounds_by_inchikeys(
        &self,
        keys: &[String],
    ) -> BoxedFuture<'_, Result<HashMap<String, WikidataCompound>, CurationError>> {
        self.count("compound-batch");
        let known = self.known.clone();
        let keys: Vec<String> = keys.to_vec();
        Box::pin(async move {
            Ok(keys
                .iter()
                .filter(|key| known.as_ref().is_none_or(|known| known.contains(*key)))
                .map(|key| {
                    (
                        key.clone(),
                        WikidataCompound {
                            qid: format!("Q{}", key.len()),
                            canonical_smiles: None,
                            isomeric_smiles: None,
                            inchi: None,
                            formula: None,
                            mass: None,
                        },
                    )
                })
                .collect())
        })
    }

    fn existing_occurrences(
        &self,
        _pairs: &[OccurrencePair],
    ) -> BoxedFuture<'_, Result<HashSet<OccurrencePair>, CurationError>> {
        self.count("occurrence-batch");
        Box::pin(async { Ok(HashSet::new()) })
    }

    fn existing_occurrences_with_ref(
        &self,
        _triples: &[OccurrenceTriple],
    ) -> BoxedFuture<'_, Result<HashSet<OccurrenceTriple>, CurationError>> {
        self.count("occurrence-with-ref-batch");
        Box::pin(async { Ok(HashSet::new()) })
    }
}

/// A row's keys, without the toolkit: the shape the pure half takes.
fn keys<'a>(
    inchikey: Option<&'a str>,
    taxon: Option<&'a str>,
    doi: Option<&'a str>,
) -> RowKeys<'a> {
    RowKeys {
        inchikey: inchikey.map(str::to_string),
        taxon,
        doi,
    }
}

/// The load-bearing assertion of this module: **one** request per question,
/// whatever the row count.
///
/// Every question was once asked per row -- four POSTs per row, so a
/// two-hundred-row import was up to eight hundred requests to a shared public
/// endpoint. What must stay fixed is the multiplier, not the total, so the
/// test runs the same prefetch at two row counts and asserts the request
/// count did not move.
#[test]
fn a_prefetch_costs_the_same_however_many_rows_there_are() {
    let taxa = HashMap::from([("gentiana lutea".to_string(), "Q158572".to_string())]);
    let mut counts = Vec::new();
    for row_count in [3usize, 200] {
        // The InChIKeys are owned here so the keys can borrow them: a run's
        // keys are the only borrows in this function and they all live as
        // long as the batch.
        let inchikeys: Vec<String> = (0..row_count).map(|i| format!("KEY{i}")).collect();
        let rows: Vec<RowKeys<'_>> = inchikeys
            .iter()
            .map(|key| keys(Some(key), Some("Gentiana lutea"), None))
            .collect();
        let repo = Counting::default();
        let cache = Mutex::new(OccurrenceAskCache::default());
        block_on(prefetch_for_keys(
            &rows,
            &repo,
            &taxa,
            &HashMap::new(),
            &cache,
        ))
        .expect("prefetch");
        counts.push(repo.total());
    }

    let small_count = *counts.first().unwrap_or(&usize::MAX);
    let large_count = *counts.get(1).unwrap_or(&usize::MAX);
    assert_eq!(
        small_count, large_count,
        "a 200-row run asked {large_count} times and a 3-row run asked \
         {small_count}: the cost scales with the rows, which is the thing \
         this module exists to stop"
    );
    assert!(
        large_count <= 2,
        "expected one compound query and one occurrence query, got {counts:?}"
    );
}

/// The three hundredth row of a real import adds no request, and neither does
/// a row that repeats a pair the batch already contains.
#[test]
fn a_repeated_pair_is_asked_once() {
    // A paper's table of one compound across twenty references, or the same
    // organism listed twice: very ordinary, and it must not cost twenty
    // entries in the query. The per-row cache absorbed the repeats -- after
    // twenty identical requests.
    let rows: Vec<RowKeys<'_>> = (0..20)
        .map(|_| keys(Some("SAMEKEY"), Some("Gentiana lutea"), None))
        .collect();
    let taxa = HashMap::from([("gentiana lutea".to_string(), "Q158572".to_string())]);
    let repo = Counting::default();
    let cache = Mutex::new(OccurrenceAskCache::default());

    block_on(prefetch_for_keys(
        &rows,
        &repo,
        &taxa,
        &HashMap::new(),
        &cache,
    ))
    .expect("prefetch");

    assert_eq!(
        crate::features::curation::services::occurrence_cache::cached_answer_count(&cache),
        1,
        "one distinct pair, so one cached answer"
    );
}

/// Including the misses: a cache of hits only sends the row loop to the
/// network for every pair Wikidata does *not* have, which is most of them.
#[test]
fn a_batch_answer_fills_the_cache_for_the_rows_that_follow() {
    let rows = [keys(Some("KEY"), Some("Gentiana lutea"), None)];
    let taxa = HashMap::from([("gentiana lutea".to_string(), "Q158572".to_string())]);
    let repo = Counting::default();
    let cache = Mutex::new(OccurrenceAskCache::default());

    block_on(prefetch_for_keys(
        &rows,
        &repo,
        &taxa,
        &HashMap::new(),
        &cache,
    ))
    .expect("prefetch");

    assert_eq!(
        crate::features::curation::services::occurrence_cache::cached_answer_count(&cache),
        1,
        "the one pair asked about should be answered in the cache, missing or not"
    );
}

/// A row with nothing to ask about contributes nothing to the query.
///
/// Three ways to have nothing: the structure did not convert, the compound is
/// not in Wikidata, or the taxon did not resolve. All three are ordinary in a
/// real import, and each one used to be a request that came back empty.
#[test]
fn a_row_with_nothing_to_ask_is_not_asked_about() {
    let taxa = HashMap::from([("gentiana lutea".to_string(), "Q158572".to_string())]);

    // `KNOWN` exists in Wikidata; `UNKNOWN` does not. The other two rows
    // differ in what they name rather than whether the compound is there.
    for row in [
        keys(None, Some("Gentiana lutea"), None),
        keys(Some("UNKNOWN"), Some("Gentiana lutea"), None),
        keys(Some("KNOWN"), Some("Gentianaceae"), None),
        keys(Some("KNOWN"), None, None),
    ] {
        let repo = Counting::knowing(&["KNOWN"]);
        let cache = Mutex::new(OccurrenceAskCache::default());
        block_on(prefetch_for_keys(
            &[row],
            &repo,
            &taxa,
            &HashMap::new(),
            &cache,
        ))
        .expect("prefetch");
        assert_eq!(
            crate::features::curation::services::occurrence_cache::cached_answer_count(&cache),
            0,
            "nothing was askable, so nothing should be cached"
        );
    }
}
