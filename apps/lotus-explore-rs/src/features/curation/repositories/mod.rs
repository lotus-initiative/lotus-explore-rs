// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

mod wikidata;

use lotus_curation::{CurationError, WikidataCompound};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

pub use wikidata::WikidataKnowledgeRepository;

/// Type alias for boxed async results. Avoids `async-trait` macro expansion bloat.
pub type BoxedFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// Resolve or create taxon result: (`qid`, `quickstatements_lines`).
pub type ResolveTaxonResult = Result<(Option<String>, Vec<String>), CurationError>;

/// Stable data-access boundary for curation orchestration and enrichment.
/// Object-safe repository trait for querying and mutating Wikidata knowledge.
pub trait CurationKnowledgeRepository: Send + Sync {
    /// Fetch a chemical compound by `InChIKey` from Wikidata.
    /// Returns `None` if no compound with that key exists; errors indicate network/parse issues.
    fn fetch_compound_by_inchikey(
        &self,
        inchikey: &str,
    ) -> BoxedFuture<'_, Result<Option<WikidataCompound>, CurationError>>;

    /// Resolve or create a taxon entity by name.
    /// If `pre_resolved_qid` is provided and valid, returns it immediately.
    fn resolve_or_create_taxon(
        &self,
        name: &str,
        pre_resolved_qid: Option<&str>,
    ) -> BoxedFuture<'_, ResolveTaxonResult>;

    /// Resolve a reference (publication) by DOI to a Wikidata QID.
    fn resolve_reference_qid(
        &self,
        doi: &str,
    ) -> BoxedFuture<'_, Result<Option<String>, CurationError>>;

    /// Check if a compound has a taxon occurrence with a specific reference (all three linked).
    fn compound_has_taxon_with_ref(
        &self,
        compound_qid: &str,
        taxon_qid: &str,
        ref_qid: &str,
    ) -> BoxedFuture<'_, Result<bool, CurationError>>;

    /// Check if a compound has a taxon occurrence (any reference).
    fn compound_has_taxon(
        &self,
        compound_qid: &str,
        taxon_qid: &str,
    ) -> BoxedFuture<'_, Result<bool, CurationError>>;

    /// Batch-resolve multiple taxon names to Wikidata QIDs.
    fn resolve_taxon_qids_batch(
        &self,
        names: &[String],
    ) -> BoxedFuture<'_, Result<HashMap<String, String>, CurationError>>;

    /// Batch-resolve multiple DOIs to Wikidata reference QIDs.
    fn resolve_reference_qids_batch(
        &self,
        dois: &[String],
    ) -> BoxedFuture<'_, Result<HashMap<String, String>, CurationError>>;
}
