// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Characterization tests pinning export pipelines, metadata JSON, filename
//! formatting, and REST API request wire serialization prior to crate extraction.

#![allow(
    unused_crate_dependencies,
    clippy::expect_used,
    reason = "integration test uses crate dependencies selectively and unwraps test fixtures"
)]

use lotus_explore_core::api::dto::SearchRequest;
use lotus_explore_core::export::archive::{ArchiveArtifacts, write_archive};
use lotus_explore_core::export::filename::generate_filename;
use lotus_explore_core::export::metadata::{MetadataInputs, SparqlEndpoint, build_metadata_json};
use lotus_explore_core::export::pipeline::{EXPORT_QUERY_COUNT, run_lotus_exporter};
use lotus_model::{ElementState, SearchCriteria, SmilesSearchType};
use lotus_query::ExportFormat;
use lotus_search::testing::Scripted;
use lotus_search::{Endpoint, Service};

const YEAR: u16 = 2026;
const NOW: &str = "2026-03-04T05:06:07Z";

#[tokio::test]
async fn pipeline_produces_exact_frozen_csv_and_metadata() {
    let scripted = Scripted::new(vec![
        (
            200,
            "compound_inchikey,compound,taxon,reference\n\
             BSYNRYMUTXBXSQ-UHFFFAOYSA-N,http://www.wikidata.org/entity/Q18216,http://www.wikidata.org/entity/Q158572,http://www.wikidata.org/entity/Q55712842\n",
        ),
        (
            200,
            "compound_cid,compound,compound_smiles_can,compound_smiles_iso\n\
             2244,http://www.wikidata.org/entity/Q18216,CC(=O)Oc1ccccc1C(=O)O,\n",
        ),
        (
            200,
            "taxon_name,taxon,taxon_ncbi,taxon_ott,taxon_gbif,taxon_parent,taxon_rank,taxon_rank_label\n\
             Gentiana lutea,http://www.wikidata.org/entity/Q158572,4999,123,456,http://www.wikidata.org/entity/Q4998,http://www.wikidata.org/entity/Q7432,species\n",
        ),
        (
            200,
            "doi,reference,pmid,pmcid,title,date,journal,journal_label\n\
             10.1000/test,http://www.wikidata.org/entity/Q55712842,111,PMC222,Paper Title,2020-01-01,http://www.wikidata.org/entity/Q99,Journal X\n",
        ),
    ]);

    let mut phases = Vec::new();
    let (frozen_csv, metadata_csv, summary) =
        run_lotus_exporter(&scripted, Endpoint::new(Service::Qlever), NOW, YEAR, |p| {
            phases.push(p);
        })
        .await
        .expect("pipeline should succeed");

    assert_eq!(
        phases.len() as u64,
        EXPORT_QUERY_COUNT,
        "pipeline must emit exactly EXPORT_QUERY_COUNT phases"
    );
    assert_eq!(
        summary.rows_written, 1,
        "pipeline summary must count 1 joined row"
    );
    assert!(
        frozen_csv.starts_with("compound_qid,compound_inchikey,compound_smiles_can,"),
        "frozen.csv header changed: {frozen_csv}"
    );
    assert!(
        frozen_csv.contains(
            "Q18216,BSYNRYMUTXBXSQ-UHFFFAOYSA-N,CC(=O)Oc1ccccc1C(=O)O,,2244,Q158572,Gentiana lutea"
        ),
        "frozen.csv joined row changed: {frozen_csv}"
    );
    assert!(
        metadata_csv.contains("date_created,2026-03-04T05:06:07Z\n")
            && metadata_csv.contains("rows_written,1\n"),
        "frozen_metadata.csv changed: {metadata_csv}"
    );
}

#[test]
fn write_archive_produces_deterministic_manifest_and_checksums() {
    let artifacts: ArchiveArtifacts = write_archive(
        NOW,
        "lotus-2026-03-04.tar.gz",
        "https://qlever.dev/api/wikidata",
        1,
        &[],
        &[],
        &[],
    )
    .expect("write_archive should succeed");

    let manifest: serde_json::Value =
        serde_json::from_str(&artifacts.manifest).expect("manifest is valid JSON");
    assert_eq!(
        manifest.get("archive_format"),
        Some(&serde_json::json!("lotus-wikidata-smiles/v1")),
        "manifest archive_format changed"
    );
    assert_eq!(
        manifest.get("fetched_at"),
        Some(&serde_json::json!(NOW)),
        "manifest fetched_at changed"
    );
    assert!(
        artifacts.checksums.contains("  lotus-2026-03-04.tar.gz\n")
            && artifacts
                .checksums
                .contains("  lotus-wikidata-smiles.manifest.json\n"),
        "checksums format changed: {}",
        artifacts.checksums
    );
    assert!(
        artifacts.archive.starts_with(&[0x1f, 0x8b]),
        "archive must be gzip-compressed"
    );
}

fn sample_criteria() -> SearchCriteria {
    let mut criteria = SearchCriteria {
        taxon: " Gentiana lutea ".into(),
        reference: " Q55712842 ".into(),
        structure: "\n  Mrv\n\n  0  0  0  0  0  0            999 V3000\nM  END\n".into(),
        structure_search: SmilesSearchType::Similarity,
        structure_threshold: 0.85,
        mass_min: 150.0,
        mass_max: 400.0,
        year_min: 2010,
        year_max: 2024,
        formula_enabled: true,
        formula_exact: " C9H8O4 ".into(),
        f_state: ElementState::Required,
        ..SearchCriteria::up_to_year(YEAR)
    };
    criteria.taxon_names.accepted_synonyms = false;
    criteria
}

#[test]
fn api_dto_wire_contract_is_pinned() {
    let criteria = sample_criteria();

    let req = SearchRequest::from_criteria(&criteria, 250, true, YEAR);
    let wire = serde_json::to_value(&req).expect("SearchRequest serializes");
    assert_eq!(
        wire.get("taxon"),
        Some(&serde_json::json!("Gentiana lutea")),
        "taxon field changed"
    );
    assert_eq!(
        wire.get("reference"),
        Some(&serde_json::json!("Q55712842")),
        "reference field changed"
    );
    assert_eq!(
        wire.get("taxon_accepted_synonyms"),
        Some(&serde_json::json!(false)),
        "taxon_accepted_synonyms changed"
    );
    assert_eq!(
        wire.get("taxon_basionyms"),
        None,
        "default-true toggle must be omitted on wire"
    );
    assert_eq!(
        wire.get("structure"),
        Some(&serde_json::json!(
            "\n  Mrv\n\n  0  0  0  0  0  0            999 V3000\nM  END\n"
        )),
        "molfile leading newline must survive"
    );
    assert_eq!(
        wire.get("structure_search"),
        Some(&serde_json::json!("similarity")),
        "structure_search changed"
    );
    assert_eq!(
        wire.get("structure_threshold"),
        Some(&serde_json::json!(0.85)),
        "structure_threshold changed"
    );
    assert_eq!(
        wire.get("f_state"),
        Some(&serde_json::json!("required")),
        "f_state changed"
    );
    assert_eq!(
        wire.get("limit"),
        Some(&serde_json::json!(250)),
        "limit changed"
    );
    assert_eq!(
        wire.get("include_counts"),
        Some(&serde_json::json!(true)),
        "include_counts changed"
    );
}

#[test]
fn metadata_and_filename_are_pinned() {
    let criteria = sample_criteria();
    let fname = generate_filename(&criteria, ExportFormat::Csv.extension(), YEAR, NOW);
    assert_eq!(
        fname, "20260304_lotus_Gentiana_lutea_similarity_filtered.csv",
        "filename changed"
    );

    let meta_raw = build_metadata_json(MetadataInputs {
        criteria: &criteria,
        qid: Some("Q158572"),
        number_of_records_override: Some(12),
        query_hash: "qhash",
        result_hash: "rhash",
        endpoint: SparqlEndpoint::Qlever,
        sparql_query: Some("SELECT ?c WHERE {}"),
        year: YEAR,
        date_created: NOW,
    });
    let meta: serde_json::Value = serde_json::from_str(&meta_raw).expect("metadata is JSON");
    assert_eq!(
        meta.pointer("/provenance/query_hash/value"),
        Some(&serde_json::json!("qhash")),
        "metadata query_hash changed"
    );
    assert_eq!(
        meta.pointer("/provenance/result_hash/value"),
        Some(&serde_json::json!("rhash")),
        "metadata result_hash changed"
    );
    assert_eq!(
        meta.get("numberOfRecords"),
        Some(&serde_json::json!(12)),
        "metadata numberOfRecords changed"
    );
}
