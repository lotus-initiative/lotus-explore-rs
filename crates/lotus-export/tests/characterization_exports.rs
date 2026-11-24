// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Characterization tests pinning the CLI bulk-export pipelines (`pipeline` and
//! `archive`) extracted into `crates/lotus-export`.

#![allow(
    unused_crate_dependencies,
    clippy::expect_used,
    reason = "integration test uses crate dependencies selectively and unwraps test fixtures"
)]

use lotus_export::archive::{ArchiveArtifacts, WriteArchiveInput, write_archive};
use lotus_export::pipeline::{EXPORT_QUERY_COUNT, PipelineOptions, run_lotus_exporter};
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
    let (frozen_csv, metadata_csv, summary) = run_lotus_exporter(
        &scripted,
        &PipelineOptions {
            endpoint: Endpoint::new(Service::Qlever),
            now: NOW,
            year: YEAR,
        },
        |p| {
            phases.push(p);
        },
    )
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
    let artifacts: ArchiveArtifacts = write_archive(&WriteArchiveInput {
        fetched_at: NOW,
        archive_filename: "lotus-2026-03-04.tar.gz",
        endpoint: "https://qlever.dev/api/wikidata",
        source_records: 1,
        missing_inchi_key_records: &[],
        records: &[],
        parse_errors: &[],
    })
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
