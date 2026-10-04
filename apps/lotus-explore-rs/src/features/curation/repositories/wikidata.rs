// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::{CurationKnowledgeRepository, OccurrencePair, OccurrenceTriple};
use crate::features::curation::services::wikidata;
use lotus_curation::{CurationError, WikidataCompound};
use std::collections::{HashMap, HashSet};

use super::ResolveTaxonResult;

#[derive(Debug, Default, Clone, Copy)]
pub struct WikidataKnowledgeRepository;

impl CurationKnowledgeRepository for WikidataKnowledgeRepository {
    fn fetch_compound_by_inchikey(
        &self,
        inchikey: &str,
    ) -> super::BoxedFuture<'_, Result<Option<WikidataCompound>, CurationError>> {
        let inchikey = inchikey.to_string();
        Box::pin(async move { wikidata::fetch_wikidata_compound_by_inchikey(&inchikey).await })
    }

    fn resolve_or_create_taxon(
        &self,
        name: &str,
        pre_resolved_qid: Option<&str>,
    ) -> super::BoxedFuture<'_, ResolveTaxonResult> {
        let name = name.to_string();
        let pre_resolved_qid = pre_resolved_qid.map(String::from);
        Box::pin(async move {
            wikidata::resolve_or_create_taxon(&name, pre_resolved_qid.as_deref()).await
        })
    }

    fn resolve_reference_qid(
        &self,
        doi: &str,
    ) -> super::BoxedFuture<'_, Result<Option<String>, CurationError>> {
        let doi = doi.to_string();
        Box::pin(async move { wikidata::resolve_reference_qid(&doi).await })
    }

    fn compound_has_taxon_with_ref(
        &self,
        compound_qid: &str,
        taxon_qid: &str,
        ref_qid: &str,
    ) -> super::BoxedFuture<'_, Result<bool, CurationError>> {
        let compound_qid = compound_qid.to_string();
        let taxon_qid = taxon_qid.to_string();
        let ref_qid = ref_qid.to_string();
        Box::pin(async move {
            wikidata::compound_has_taxon_with_ref(&compound_qid, &taxon_qid, &ref_qid).await
        })
    }

    fn compound_has_taxon(
        &self,
        compound_qid: &str,
        taxon_qid: &str,
    ) -> super::BoxedFuture<'_, Result<bool, CurationError>> {
        let compound_qid = compound_qid.to_string();
        let taxon_qid = taxon_qid.to_string();
        Box::pin(async move { wikidata::compound_has_taxon(&compound_qid, &taxon_qid).await })
    }

    fn resolve_taxon_qids_batch(
        &self,
        names: &[String],
    ) -> super::BoxedFuture<'_, Result<HashMap<String, String>, CurationError>> {
        let names: Vec<String> = names.to_vec();
        Box::pin(async move {
            wikidata::resolve_taxon_qids_batch(names.iter().map(String::as_str)).await
        })
    }

    fn resolve_reference_qids_batch(
        &self,
        dois: &[String],
    ) -> super::BoxedFuture<'_, Result<HashMap<String, String>, CurationError>> {
        let dois: Vec<String> = dois.to_vec();
        Box::pin(async move {
            wikidata::resolve_reference_qids_batch(dois.iter().map(String::as_str)).await
        })
    }

    fn fetch_compounds_by_inchikeys(
        &self,
        keys: &[String],
    ) -> super::BoxedFuture<'_, Result<HashMap<String, WikidataCompound>, CurationError>> {
        let keys: Vec<String> = keys.to_vec();
        Box::pin(async move {
            wikidata::fetch_compounds_by_inchikeys(keys.iter().map(String::as_str)).await
        })
    }

    fn existing_occurrences(
        &self,
        pairs: &[OccurrencePair],
    ) -> super::BoxedFuture<'_, Result<HashSet<OccurrencePair>, CurationError>> {
        let pairs: Vec<OccurrencePair> = pairs.to_vec();
        Box::pin(
            async move { wikidata::existing_occurrences(&pairs, wikidata::OCCURRENCE_CHUNK).await },
        )
    }

    fn existing_occurrences_with_ref(
        &self,
        triples: &[OccurrenceTriple],
    ) -> super::BoxedFuture<'_, Result<HashSet<OccurrenceTriple>, CurationError>> {
        let triples: Vec<OccurrenceTriple> = triples.to_vec();
        Box::pin(async move {
            wikidata::existing_occurrences_with_ref(&triples, wikidata::OCCURRENCE_CHUNK).await
        })
    }
}
