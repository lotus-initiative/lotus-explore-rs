// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::features::curation::repositories::{
    CurationKnowledgeRepository, WikidataKnowledgeRepository,
};
use crate::features::curation::services::occurrence_cache::OccurrenceAskCache;
use crate::features::curation::services::{curate_single_row, inputs, pipeline};
#[cfg(test)]
use crate::features::curation::services::{
    extract_exact_mass_from_json, extract_formula_from_inchi, normalize_formula_for_wikidata,
    qs_mass_statement,
};
use crate::i18n::Locale;
use lotus_curation as domain;
use std::sync::{Arc, Mutex};

pub use lotus_curation::{
    CurationError, CurationErrorKind, CurationInputRow, CurationResultRow, CurationStatus,
    QuickStatementsBundle,
};

#[cfg(test)]
mod contract_tests;

#[path = "curation/share_links.rs"]
mod share_links;
#[cfg(test)]
use share_links::{CURATION_ROWS_PARAM, curation_rows_from_query_params};
pub use share_links::{
    build_curation_share_url, initial_curation_autorun_from_url, initial_curation_rows_from_url,
};

pub fn example_rows() -> Vec<CurationInputRow> {
    inputs::example_rows()
}

pub fn parse_tsv_rows(tsv: &str) -> Result<Vec<CurationInputRow>, CurationError> {
    inputs::parse_tsv_rows(tsv)
}

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
    crate::features::curation::services::prefetch_knowledge(
        &rows,
        repository.as_ref(),
        prefetched_taxa.as_ref(),
        prefetched_references.as_ref(),
        &occurrence_ask_cache,
    )
    .await?;

    pipeline::curate_rows(
        locale,
        rows,
        {
            let prefetched_taxa = Arc::clone(&prefetched_taxa);
            let prefetched_references = Arc::clone(&prefetched_references);
            let occurrence_ask_cache = Arc::clone(&occurrence_ask_cache);
            let repository = Arc::clone(&repository);
            move |locale, row| {
                curate_single_row(
                    locale,
                    row,
                    Arc::clone(&repository),
                    Arc::clone(&prefetched_taxa),
                    Arc::clone(&prefetched_references),
                    Arc::clone(&occurrence_ask_cache),
                )
            }
        },
        row_uniqueness_key,
    )
    .await
}

pub fn build_quickstatements_bundle(results: &[CurationResultRow]) -> QuickStatementsBundle {
    domain::build_quickstatements_bundle(results)
}

pub fn row_uniqueness_key(row: &CurationInputRow) -> String {
    inputs::row_uniqueness_key(row)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::indexing_slicing)]

    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn parse_tsv_supports_expected_headers() {
        let tsv = "name\tsmiles\torganism\tdoi\nA\tCCO\tTaxon\thttps://doi.org/10.1/x\n";
        let rows = parse_tsv_rows(tsv).expect("tsv parse");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "A");
        assert_eq!(rows[0].smiles, "CCO");
        assert_eq!(rows[0].taxon.as_deref(), Some("Taxon"));
        assert_eq!(rows[0].doi.as_deref(), Some("10.1/X"));
    }

    #[test]
    fn extract_formula_reads_inchi_main_layer() {
        assert_eq!(
            extract_formula_from_inchi("InChI=1S/C8H10N4O2/c1-10-4-9-6-5(10)7(13)12(3)8(14)11(6)2"),
            Some("C8H10N4O2".to_string())
        );
    }

    #[test]
    fn normalize_formula_produces_subscript_digits() {
        assert_eq!(normalize_formula_for_wikidata("C8H10N4O2"), "C₈H₁₀N₄O₂");
    }

    #[test]
    fn normalize_formula_passes_through_non_digit_chars() {
        assert_eq!(normalize_formula_for_wikidata("C10H12F3N5O"), "C₁₀H₁₂F₃N₅O");
    }

    #[test]
    fn row_key_normalizes_structure_taxon_and_doi() {
        let row = CurationInputRow {
            name: "compound A".to_string(),
            smiles: " cco ".to_string(),
            taxon: Some("  Voacanga africana ".to_string()),
            doi: Some("https://doi.org/10.1000/abc".to_string()),
        };
        assert_eq!(
            row_uniqueness_key(&row),
            "CCO\tvoacanga africana\t10.1000/abc"
        );
    }

    #[test]
    fn qs_mass_uses_unit_not_qunit() {
        // Unit syntax in QS is U<QID>, not UQ<QID>.
        let stmt = qs_mass_statement("LAST", 495.20268);
        assert!(stmt.contains("U483261"), "expected U483261 but got: {stmt}");
        assert!(
            !stmt.contains("UQ483261"),
            "must not contain UQ483261: {stmt}"
        );
    }

    #[test]
    fn extract_exact_mass_from_nested_json_dict() {
        let payload = serde_json::json!({
            "CCO": {
                "exact_molecular_weight": 46.04186,
                "molecular_formula": "C2H6O"
            }
        });
        assert_eq!(extract_exact_mass_from_json(&payload), Some(46.04186));
    }

    #[test]
    fn extract_exact_mass_from_nested_json_array() {
        let payload = serde_json::json!({
            "results": [
                {"foo": "bar"},
                {"descriptors": {"exact_molecular_weight": 180.06339}}
            ]
        });
        assert_eq!(extract_exact_mass_from_json(&payload), Some(180.06339));
    }

    #[test]
    fn extract_exact_mass_from_string_number_with_grouping() {
        let payload = serde_json::json!({
            "exact_molecular_weight": "1,234.5678"
        });
        assert_eq!(extract_exact_mass_from_json(&payload), Some(1234.5678));
    }

    #[test]
    fn curation_share_params_roundtrip_rows() {
        let rows = vec![
            CurationInputRow {
                name: "Compound A".to_string(),
                smiles: "CCO".to_string(),
                taxon: Some("Gentiana lutea".to_string()),
                doi: Some("10.1000/ABC".to_string()),
            },
            CurationInputRow {
                name: "Compound B".to_string(),
                smiles: "C1=CC=CC=C1".to_string(),
                taxon: None,
                doi: None,
            },
        ];
        let mut params = BTreeMap::new();
        params.insert(
            CURATION_ROWS_PARAM.to_string(),
            serde_json::to_string(&rows).expect("rows json"),
        );
        assert_eq!(curation_rows_from_query_params(&params), rows);
    }

    #[test]
    fn curation_share_url_contains_route_and_autorun() {
        let rows = vec![CurationInputRow {
            name: "Compound A".to_string(),
            smiles: "CCO".to_string(),
            taxon: None,
            doi: None,
        }];
        let url = build_curation_share_url(&rows, Locale::Fr, true).expect("share url");
        assert!(url.starts_with("/curation?"));
        assert!(!url.contains("view="));
        assert!(url.contains("lang=fr"));

        assert!(url.contains("curation_run=true"));
        assert!(url.contains("curation_rows="));
    }

    #[test]
    fn build_quickstatements_bundle_deduplicates_dependencies_and_joins_sections() {
        let rows = vec![
            CurationResultRow {
                input: CurationInputRow {
                    name: "A".into(),
                    smiles: "C".into(),
                    taxon: None,
                    doi: None,
                },
                canonical_smiles: None,
                inchikey: None,
                inchi: None,
                formula: None,
                exact_mass: None,
                mass_warning: None,
                wikidata_qid: None,
                status: CurationStatus::NewCompound,
                note: String::new(),
                dependency_blocks: vec!["DEP-1".into(), "DEP-1".into()],
                quickstatements: vec!["MAIN-1A".into(), "MAIN-1B".into()],
            },
            CurationResultRow {
                input: CurationInputRow {
                    name: "B".into(),
                    smiles: "N".into(),
                    taxon: None,
                    doi: None,
                },
                canonical_smiles: None,
                inchikey: None,
                inchi: None,
                formula: None,
                exact_mass: None,
                mass_warning: None,
                wikidata_qid: None,
                status: CurationStatus::NewCompound,
                note: String::new(),
                dependency_blocks: vec!["DEP-1".into(), "DEP-2".into()],
                quickstatements: vec!["MAIN-2".into()],
            },
        ];

        let bundle = build_quickstatements_bundle(&rows);
        assert_eq!(bundle.dependencies.as_ref(), "DEP-1\n\nDEP-2");
        assert_eq!(bundle.main.as_ref(), "MAIN-1A\nMAIN-1B\n\nMAIN-2");
    }
}
