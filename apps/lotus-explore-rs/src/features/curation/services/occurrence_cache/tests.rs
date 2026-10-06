// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `occurrence_cache`, in their own file.

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

    // Two trait items, so two bodies are the trait's shape rather than a
    // copy: `resolve_taxon_qids_batch` cannot call
    // `resolve_reference_qids_batch`, being separate items of the same trait
    // and neither a specialisation of the other. A shared helper would be
    // three functions to express two empty stubs.
    //
    // The marker goes last because it must be the line immediately above the
    // function; prose in between makes the pair get reported anyway, which
    // looks like a broken gate.
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

    let first =
        block_on(compound_has_taxon_cached(&repo, &cache, "Q1", "Q2")).expect("first ask result");
    let second =
        block_on(compound_has_taxon_cached(&repo, &cache, "Q1", "Q2")).expect("second ask result");

    assert!(!first);
    assert!(!second);
    assert_eq!(*repo.ask_calls.lock().expect("counter lock"), 1);
}
