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
#[path = "prefetch/tests.rs"]
mod tests;
