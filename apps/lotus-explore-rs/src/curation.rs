// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::curation::repositories::CurationKnowledgeRepository;
use crate::features::curation::repositories::wikidata::WikidataKnowledgeRepository;
use crate::features::curation::services::enrichment::curate_single_row;
use crate::features::curation::services::occurrence_cache::OccurrenceAskCache;
use crate::features::curation::services::pipeline;
use crate::i18n::Locale;
use lotus_curation::{CurationError, CurationInputRow, CurationResultRow, QuickStatementsBundle};
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod contract_tests;

#[path = "curation/share_links.rs"]
pub mod share_links;

// The curation pipeline drives Dioxus signals and reqwest (WASM) futures,
// which are `!Send`; see the `repositories` design note.
#[allow(clippy::future_not_send)]
pub async fn curate_rows(
    locale: Locale,
    rows: Vec<CurationInputRow>,
) -> Result<(Vec<CurationResultRow>, QuickStatementsBundle), CurationError> {
    let repository: Arc<dyn CurationKnowledgeRepository> = Arc::new(WikidataKnowledgeRepository);
    let taxon_names = rows
        .iter()
        .filter_map(|row| row.taxon.clone())
        .collect::<Vec<_>>();
    let doi_values = rows
        .iter()
        .filter_map(|row| row.doi.clone())
        .collect::<Vec<_>>();

    let prefetched_taxa = Arc::new(repository.resolve_taxon_qids_batch(&taxon_names).await?);
    let prefetched_references =
        Arc::new(repository.resolve_reference_qids_batch(&doi_values).await?);
    let occurrence_ask_cache = Arc::new(Mutex::new(OccurrenceAskCache::default()));

    // The remaining per-row questions, asked once for the whole batch.
    //
    // Everything the row loop needs from Wikidata is one of four questions, and
    // all four have a batched form. Asking them per row is what made curation
    // the heaviest thing this project does to a public endpoint: four requests
    // per row, so a 200-row import was up to 800 POSTs, and a second pass over
    // dependency rows repeated every one of them.
    //
    // The compounds need the local `InChIKey` first, so this runs after the
    // structures are converted -- `convert_structure` is RDKit in the browser
    // and costs no network at all.
    let prefetched_compounds = Arc::new(
        crate::features::curation::services::prefetch::prefetch_knowledge(
            &rows,
            repository.as_ref(),
            prefetched_taxa.as_ref(),
            prefetched_references.as_ref(),
            &occurrence_ask_cache,
        )
        .await?,
    );

    pipeline::curate_rows(
        locale,
        rows,
        {
            let prefetched_compounds = Arc::clone(&prefetched_compounds);
            let prefetched_taxa = Arc::clone(&prefetched_taxa);
            let prefetched_references = Arc::clone(&prefetched_references);
            let occurrence_ask_cache = Arc::clone(&occurrence_ask_cache);
            let repository = Arc::clone(&repository);
            move |locale, row| {
                curate_single_row(
                    locale,
                    row,
                    Arc::clone(&repository),
                    Arc::clone(&prefetched_compounds),
                    Arc::clone(&prefetched_taxa),
                    Arc::clone(&prefetched_references),
                    Arc::clone(&occurrence_ask_cache),
                )
            }
        },
        lotus_curation::row_uniqueness_key,
    )
    .await
}

#[cfg(test)]
#[path = "curation/tests.rs"]
mod tests;
