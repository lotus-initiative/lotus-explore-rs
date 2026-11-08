// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Golden tests for the LOTUS exporter pipeline, driven against a scripted
//! transport instead of the network.

use std::collections::HashSet;
use std::path::PathBuf;

use lotus_search::testing::Scripted;
use lotus_search::{Endpoint, Service};

use super::*;

const NOW: &str = "2026-03-04T05:06:07Z";
const YEAR: u16 = 2026;

/// One scripted reply carrying a CSV payload.
fn csv(status: u16, body: &str) -> (u16, &str) {
    (status, body)
}

/// A triplet payload: the compound-taxon-reference core table.
fn triplet_csv() -> String {
    "compound_inchikey,compound,taxon,reference\n\
IK100,http://www.wikidata.org/entity/Q1000,http://www.wikidata.org/entity/Q2000,http://www.wikidata.org/entity/Q3000\n\
,http://www.wikidata.org/entity/Q1001,http://www.wikidata.org/entity/Q2001,\n\
"
    .to_string()
}

/// Consolidated compound properties, including a second row for `Q1000` with
/// an empty `compound_smiles_can` cell so empty optional cells never clobber a
/// populated field.
fn compounds_csv() -> String {
    "compound_cid,compound,compound_smiles_can,compound_smiles_iso\n\
1234,http://www.wikidata.org/entity/Q1000,CCO,\n\
,http://www.wikidata.org/entity/Q1000,,C[C@H](O)O\n\
,http://www.wikidata.org/entity/Q1001,CC,\n\
"
    .to_string()
}

/// Consolidated taxon properties covering every taxon column.
fn taxa_csv() -> String {
    "taxon_name,taxon,taxon_ncbi,taxon_ott,taxon_gbif,taxon_parent,taxon_rank,taxon_rank_label\n\
Gentiana lutea,http://www.wikidata.org/entity/Q2000,4204,OTT1,GBIF1,http://www.wikidata.org/entity/Q10,http://www.wikidata.org/entity/Q7432,species\n\
Taxon without QID,http://www.wikidata.org/entity/Q2001,,,,,,\n\
"
    .to_string()
}

/// Consolidated reference properties covering every bibliographic column.
fn references_csv() -> String {
    "doi,reference,pmid,pmcid,title,date,journal,journal_label\n\
10.1/a,http://www.wikidata.org/entity/Q3000,999,PMC1,The title,2026-01-01,http://www.wikidata.org/entity/Q50,Journal Name\n\
"
    .to_string()
}

/// A minimal table with a header and no rows.
fn empty_references_csv() -> String {
    "reference,doi\n".to_string()
}

fn script(replies: Vec<(u16, &str)>) -> Scripted {
    Scripted::new(replies)
}

/// The endpoint the scripted transport pretends to be.
fn endpoint() -> Endpoint {
    Endpoint::new(Service::Qlever)
}

/// A temp dir whose path outlives the call, for writing the returned CSV
/// strings to disk (keeps `tempfile` exercised and tests real file I/O).
fn out_dir() -> PathBuf {
    tempfile::TempDir::new().expect("temp dir").keep()
}

#[tokio::test]
async fn pipeline_serializes_frozen_csv_and_metadata() {
    let http = script(vec![
        csv(200, &triplet_csv()),
        csv(200, &compounds_csv()),
        csv(200, &taxa_csv()),
        csv(200, &references_csv()),
    ]);

    let (frozen_csv, metadata_csv, summary) =
        run_lotus_exporter(&http, endpoint(), NOW, YEAR, |_| {})
            .await
            .expect("the pipeline runs");

    assert_eq!(summary.triplets, 2);
    assert_eq!(summary.compounds, 2);
    assert_eq!(summary.taxa, 2);
    assert_eq!(summary.references, 1);
    assert_eq!(summary.rows_written, 2);

    let mut lines = frozen_csv.lines();
    let header = lines.next().expect("a header");
    assert_eq!(
        header,
        "compound_qid,compound_inchikey,compound_smiles_can,compound_smiles_iso,\
compound_cid,taxon_qid,taxon_name,taxon_ncbi,taxon_ott,taxon_gbif,taxon_parent,\
taxon_rank,taxon_rank_label,reference_qid,reference_title,reference_doi,\
reference_pmid,reference_pmcid,reference_date,reference_journal,\
reference_journal_label"
    );
    let first = lines.next().expect("row 1");
    assert_eq!(
        first,
        "Q1000,IK100,CCO,C[C@H](O)O,1234,Q2000,Gentiana lutea,4204,OTT1,GBIF1,\
http://www.wikidata.org/entity/Q10,http://www.wikidata.org/entity/Q7432,species,\
Q3000,The title,10.1/a,999,PMC1,2026-01-01,http://www.wikidata.org/entity/Q50,Journal Name"
    );

    assert!(metadata_csv.contains("date_created,2026-03-04T05:06:07Z"));
    assert!(metadata_csv.contains("triplets,2"));
    assert!(metadata_csv.contains("rows_written,2"));

    // Writing the returned strings to disk works end to end.
    let dir = out_dir();
    let frozen_path = dir.join("frozen.csv");
    std::fs::write(&frozen_path, &frozen_csv).expect("write frozen.csv");
    assert!(frozen_path.exists());
}

#[tokio::test]
async fn pipeline_tolerates_missing_optional_tables() {
    let http = script(vec![
        csv(200, &triplet_csv()),
        csv(200, &compounds_csv()),
        csv(200, &taxa_csv()),
        csv(200, &empty_references_csv()),
    ]);

    let (frozen_csv, _, summary) = run_lotus_exporter(&http, endpoint(), NOW, YEAR, |_| {})
        .await
        .expect("the pipeline runs");

    assert_eq!(summary.rows_written, 2);
    // Q1001 has no inchikey, no optional compound/taxon/reference cells: empty cells, not a missing row.
    let rows: HashSet<String> = frozen_csv.lines().skip(1).map(str::to_string).collect();
    assert!(
        rows.iter()
            .any(|r| r.starts_with("Q1001,,CC,,,Q2001,Taxon without QID,,,,,,,,"))
    );
}

#[tokio::test]
async fn pipeline_records_every_query_in_order() {
    let http = script(vec![
        csv(200, &triplet_csv()),
        csv(200, &compounds_csv()),
        csv(200, &taxa_csv()),
        csv(200, &references_csv()),
    ]);

    let mut stages = Vec::new();
    let _ = run_lotus_exporter(&http, endpoint(), NOW, YEAR, |table| stages.push(table)).await;
    assert_eq!(
        stages,
        [
            "compound_taxon_reference",
            "compounds",
            "taxa",
            "references",
        ]
    );
    assert_eq!(stages.len() as u64, EXPORT_QUERY_COUNT);
    let queries = http.queries();
    assert_eq!(queries.len(), 4, "one fetch per entity table");
    assert!(
        queries
            .first()
            .expect("at least one query")
            .contains("SELECT DISTINCT ?compound ?compound_inchikey ?taxon ?reference")
    );
    assert!(
        queries
            .get(1)
            .expect("compounds query")
            .contains("SELECT ?compound ?compound_smiles_can ?compound_smiles_iso ?compound_cid")
    );
    assert!(
        queries
            .get(2)
            .expect("taxa query")
            .contains("SELECT ?taxon ?taxon_name ?taxon_ncbi ?taxon_ott ?taxon_gbif ?taxon_parent ?taxon_rank ?taxon_rank_label")
    );
    assert!(
        queries
            .last()
            .expect("references query")
            .contains("SELECT ?reference ?doi ?pmid ?pmcid ?title ?date ?journal ?journal_label")
    );
}

#[tokio::test]
async fn pipeline_propagates_fetch_errors() {
    // `400` is not retryable, so the `Fetch` error propagates straight through
    // the retry loop rather than being swallowed by a second attempt.
    let http = script(vec![(400, "bad request")]);

    let mut stages = Vec::new();
    let err = run_lotus_exporter(&http, endpoint(), NOW, YEAR, |table| stages.push(table))
        .await
        .unwrap_err();
    assert_eq!(stages, ["compound_taxon_reference"]);
    assert!(matches!(err, ExportError::Fetch(_)));
}
