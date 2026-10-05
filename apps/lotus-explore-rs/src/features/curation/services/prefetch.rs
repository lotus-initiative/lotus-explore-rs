// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! What a whole curation run asks Wikidata **once**, before it walks the rows.
//!
//! Four per-row questions -- does this `InChIKey` exist, what is this taxon's
//! item, what is this DOI's item, is this occurrence recorded (optionally *by
//! this paper*) -- each with a batched form, so a run asks each once: five
//! queries per run, whatever the row count. Four POSTs per row made a
//! two-hundred-row import up to eight hundred requests to a shared public
//! endpoint, and a second pass over the dependency rows repeated every one.
//!
//! Lives here rather than in the row loop because the keys it batches on are the
//! ones the row loop looks up and those are private to this module tree. A
//! prefetch keyed differently is a cache that never hits, the one failure mode a
//! cache cannot report.
#![expect(
    clippy::future_not_send,
    reason = "the RDKit bridge holds a Dioxus signal across its await; see enrichment.rs"
)]

use super::occurrence_cache::{OccurrenceAskCache, record_pairs_cached, record_triples_cached};
use super::{CurationError, CurationInputRow, helpers::normalize_doi, wikidata};
use crate::features::curation::repositories::CurationKnowledgeRepository;
use lotus_curation::WikidataCompound;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// The two questions the prefetch answers, as the pairs they are asked over.
#[derive(Default)]
struct Asked {
    /// `(compound, taxon)`: recorded by any reference.
    pairs: Vec<(String, String)>,
    /// `(compound, taxon, reference)`: recorded by this paper.
    triples: Vec<(String, String, String)>,
}

impl Asked {
    /// Ask both, and record every answer in the cache the row loop reads.
    ///
    /// Misses record as `false`, matching the per-pair `ASK`. A cache of hits
    /// only would send a run where every compound already exists back to the
    /// network once per row.
    async fn answer_into(
        self,
        repository: &dyn CurationKnowledgeRepository,
        cache: &Mutex<OccurrenceAskCache>,
    ) -> Result<(), CurationError> {
        if !self.pairs.is_empty() {
            let existing = repository
                .existing_occurrences(&self.pairs)
                .await
                .map_err(|e| annotate(&e, "occurrence lookup"))?;
            for pair in &self.pairs {
                record_pairs_cached(cache, [pair.clone()], existing.contains(pair));
            }
        }
        if !self.triples.is_empty() {
            let existing = repository
                .existing_occurrences_with_ref(&self.triples)
                .await
                .map_err(|e| annotate(&e, "occurrence-with-reference lookup"))?;
            for triple in &self.triples {
                record_triples_cached(cache, [triple.clone()], existing.contains(triple));
            }
        }
        Ok(())
    }
}

/// Name the phase in the error, because "the query failed" is useless to somebody
/// looking at a five-query run and wondering which one it was.
fn annotate(error: &CurationError, phase: &str) -> CurationError {
    CurationError::Http(format!("{phase}: {error}"))
}

/// One row, reduced to what Wikidata is asked about it.
///
/// Separate from the asking because the *asking* needs a runtime (RDKit runs
/// through a Dioxus signal) and every question here is decided from these three
/// values, which are testable without a browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RowKeys<'a> {
    /// The `InChIKey` the structure converted to, or `None` if it did not convert.
    pub(super) inchikey: Option<String>,
    /// The taxon as written, which is a name and not yet an item.
    pub(super) taxon: Option<&'a str>,
    /// The DOI as written.
    pub(super) doi: Option<&'a str>,
}

/// Ask everything the row loop would otherwise ask one row at a time.
///
/// Returns the compounds it found, keyed by `InChIKey`, because the row loop has
/// to agree with it about which key maps to which item -- so it is returned rather
/// than hidden, and the caller puts it where the rows will read it.
pub async fn prefetch_knowledge(
    rows: &[CurationInputRow],
    repository: &dyn CurationKnowledgeRepository,
    taxa: &HashMap<String, String>,
    references: &HashMap<String, String>,
    cache: &Mutex<OccurrenceAskCache>,
) -> Result<HashMap<String, WikidataCompound>, CurationError> {
    let keys = row_keys(rows).await;
    prefetch_for_keys(&keys, repository, taxa, references, cache).await
}

/// Convert every row's structure, which is the only part that needs a runtime.
async fn row_keys(rows: &[CurationInputRow]) -> Vec<RowKeys<'_>> {
    let mut keys = Vec::with_capacity(rows.len());
    for row in rows {
        let inchikey = super::enrichment::convert_structure(&row.smiles)
            .await
            .ok()
            .map(|structure| structure.inchikey);
        keys.push(RowKeys {
            inchikey,
            taxon: row.taxon.as_deref(),
            doi: row.doi.as_deref(),
        });
    }
    keys
}

/// The whole prefetch, over keys rather than rows.
pub(super) async fn prefetch_for_keys(
    keys: &[RowKeys<'_>],
    repository: &dyn CurationKnowledgeRepository,
    taxa: &HashMap<String, String>,
    references: &HashMap<String, String>,
    cache: &Mutex<OccurrenceAskCache>,
) -> Result<HashMap<String, WikidataCompound>, CurationError> {
    let compounds = prefetch_compounds(keys, repository).await?;
    let asked = collect_asks(keys, &compounds, taxa, references);
    asked.answer_into(repository, cache).await?;
    Ok(compounds)
}

/// One query for every `InChIKey` in the batch.
///
/// Converted here because the `InChIKey` is what the batch keys on and the
/// conversion is local: RDKit in the browser, no network. A row whose structure
/// cannot be read contributes no key, so this cannot precede conversion.
async fn prefetch_compounds(
    rows: &[RowKeys<'_>],
    repository: &dyn CurationKnowledgeRepository,
) -> Result<HashMap<String, WikidataCompound>, CurationError> {
    let mut keys: Vec<String> = Vec::with_capacity(rows.len());
    for row in rows {
        // A structure the toolkit cannot read contributes no key: there is
        // nothing to ask Wikidata about a molecule this run does not have.
        if let Some(inchikey) = &row.inchikey {
            keys.push(inchikey.clone());
        }
    }
    repository
        .fetch_compounds_by_inchikeys(&keys)
        .await
        .map_err(|e| annotate(&e, "compound lookup"))
}

/// Work out which occurrence questions this batch can actually ask.
///
/// A compound Wikidata does not have, or a taxon the spreadsheet did not resolve,
/// has no occurrence to look for: including either would make the query larger
/// for answers nobody reads.
fn collect_asks(
    rows: &[RowKeys<'_>],
    compounds: &HashMap<String, WikidataCompound>,
    taxa: &HashMap<String, String>,
    references: &HashMap<String, String>,
) -> Asked {
    let mut asked = Asked::default();
    let mut seen_pairs: HashSet<(String, String)> = HashSet::new();
    let mut seen_triples: HashSet<(String, String, String)> = HashSet::new();

    for row in rows {
        let Some(compound) = row.inchikey.as_ref().and_then(|key| compounds.get(key)) else {
            continue;
        };
        let Some(taxon_name) = row.taxon.map(str::trim).filter(|t| !t.is_empty()) else {
            continue;
        };
        // `None` for a genus or a name Wikidata does not have: the row loop will
        // create the taxon, and there is nothing to ask about until it exists.
        let Some(taxon) = wikidata::normalize_taxon_lookup(taxon_name).and_then(|k| taxa.get(&k))
        else {
            continue;
        };
        let reference = row
            .doi
            .and_then(normalize_doi)
            .and_then(|doi| references.get(&doi));

        if let Some(reference) = reference {
            let triple = (compound.qid.clone(), taxon.clone(), reference.clone());
            if seen_triples.insert(triple.clone()) {
                asked.triples.push(triple);
            }
        } else {
            let pair = (compound.qid.clone(), taxon.clone());
            if seen_pairs.insert(pair.clone()) {
                asked.pairs.push(pair);
            }
        }
    }
    asked
}

#[cfg(test)]
mod tests {
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
}
