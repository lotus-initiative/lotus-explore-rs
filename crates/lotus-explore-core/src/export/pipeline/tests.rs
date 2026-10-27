// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::redundant_closure,
    clippy::redundant_closure_for_method_calls
)]
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
    "compound,compound_inchikey,taxon,reference\n\
http://www.wikidata.org/entity/Q1000,IK100,http://www.wikidata.org/entity/Q2000,http://www.wikidata.org/entity/Q3000\n\
http://www.wikidata.org/entity/Q1001,,http://www.wikidata.org/entity/Q2001,\n\
".to_string()
}

/// A compound canonical-SMILES payload.
fn smiles_can_csv() -> String {
    "compound,compound_smiles_can\n\
http://www.wikidata.org/entity/Q1000,CCO\n\
http://www.wikidata.org/entity/Q1001,CC\n\
"
    .to_string()
}

/// A compound isomeric-SMILES payload.
fn smiles_iso_csv() -> String {
    "compound,compound_smiles_iso\n\
http://www.wikidata.org/entity/Q1000,C[C@H](O)O\n\
"
    .to_string()
}

/// A compound PubChem-CID payload.
fn cid_csv() -> String {
    "compound,compound_cid\n\
http://www.wikidata.org/entity/Q1000,1234\n\
"
    .to_string()
}

/// A taxon-name payload.
fn name_csv() -> String {
    "taxon,taxon_name\n\
http://www.wikidata.org/entity/Q2000,Gentiana lutea\n\
http://www.wikidata.org/entity/Q2001,Taxon without QID\n\
"
    .to_string()
}

/// A taxon NCBI payload.
fn ncbi_csv() -> String {
    "taxon,taxon_ncbi\n\
http://www.wikidata.org/entity/Q2000,4204\n\
"
    .to_string()
}

/// An empty payload: a table with a header but no rows.
fn empty_csv() -> String {
    "taxon,taxon_ncbi\n".to_string()
}

/// A reference title payload.
fn title_csv() -> String {
    "reference,title\n\
http://www.wikidata.org/entity/Q3000,The title\n\
"
    .to_string()
}

/// A reference DOI payload.
fn doi_csv() -> String {
    "reference,doi\n\
http://www.wikidata.org/entity/Q3000,10.1/a\n\
"
    .to_string()
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
        csv(200, &smiles_can_csv()),
        csv(200, &smiles_iso_csv()),
        csv(200, &cid_csv()),
        csv(200, &name_csv()),
        csv(200, &ncbi_csv()),
        csv(200, &empty_csv()), // ott
        csv(200, &empty_csv()), // gbif
        csv(200, &empty_csv()), // parent
        csv(200, &empty_csv()), // rank
        csv(200, &doi_csv()),
        csv(200, &empty_csv()), // pmid
        csv(200, &empty_csv()), // pmcid
        csv(200, &title_csv()),
        csv(200, &empty_csv()), // date
        csv(200, &empty_csv()), // journal
    ]);

    let (frozen_csv, metadata_csv, summary) = run_lotus_exporter(&http, endpoint(), NOW, YEAR)
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
    assert!(first.starts_with("Q1000,IK100,CCO,C[C@H](O)O,1234,"));
    assert!(first.contains(",Q2000,Gentiana lutea,4204,"));
    assert!(first.contains(",Q3000,The title,10.1/a,"));

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
        csv(200, &smiles_can_csv()),
        csv(200, &smiles_iso_csv()),
        csv(200, &cid_csv()),
        csv(200, &name_csv()),
        csv(200, &ncbi_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
    ]);

    let (frozen_csv, _, summary) = run_lotus_exporter(&http, endpoint(), NOW, YEAR)
        .await
        .expect("the pipeline runs");

    assert_eq!(summary.rows_written, 2);
    // Q1001 has no inchikey, no taxon name, no reference: empty cells, not a missing row.
    let rows: HashSet<String> = frozen_csv.lines().skip(1).map(|s| s.to_string()).collect();
    assert!(rows.iter().any(|r| r.starts_with("Q1001,,CC,,,")));
}

#[tokio::test]
async fn pipeline_records_every_query_in_order() {
    let http = script(vec![
        csv(200, &triplet_csv()),
        csv(200, &smiles_can_csv()),
        csv(200, &smiles_iso_csv()),
        csv(200, &cid_csv()),
        csv(200, &name_csv()),
        csv(200, &ncbi_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
        csv(200, &empty_csv()),
    ]);

    let _ = run_lotus_exporter(&http, endpoint(), NOW, YEAR).await;
    let queries = http.queries();
    assert_eq!(queries.len(), 16, "one fetch per table");
    assert!(queries[0].contains("SELECT DISTINCT ?compound ?compound_inchikey ?taxon ?reference"));
    assert!(
        queries
            .last()
            .expect("last query")
            .contains("?reference wdt:P1433 ?journal")
    );
}

#[tokio::test]
async fn pipeline_propagates_fetch_errors() {
    // `400` is not retryable, so the `Fetch` error propagates straight through
    // the retry loop rather than being swallowed by a second attempt.
    let http = script(vec![(400, "bad request")]);

    let err = run_lotus_exporter(&http, endpoint(), NOW, YEAR)
        .await
        .unwrap_err();
    assert!(matches!(err, ExportError::Fetch(_)));
}
