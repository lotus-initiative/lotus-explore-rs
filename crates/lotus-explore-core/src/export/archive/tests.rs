// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
#![allow(clippy::expect_used)]
//! Unit tests for `archive`.

use std::io::Read;

use flate2::read::GzDecoder;

use lotus_search::testing::Scripted;
use lotus_search::{Endpoint, Service};

use super::*;

fn endpoint() -> Endpoint {
    Endpoint::new(Service::Qlever)
}

fn json_reply(status: u16, body: &str) -> (u16, &str) {
    (status, body)
}

/// Decode a `tar.gz` archive into a map of member name → file contents.
/// Used by every test that needs to inspect archived files.
fn decode_archive(archive: &[u8]) -> std::collections::BTreeMap<String, String> {
    let mut tar = tar::Archive::new(GzDecoder::new(archive));
    let mut members = std::collections::BTreeMap::new();
    for entry in tar.entries().expect("tar entries") {
        let mut entry = entry.expect("tar entry");
        let name = entry
            .path()
            .expect("entry path")
            .to_string_lossy()
            .into_owned();
        let mut contents = String::new();
        entry.read_to_string(&mut contents).expect("read entry");
        members.insert(name, contents);
    }
    members
}

fn format_compound(qid: &str) -> String {
    format!("http://www.wikidata.org/entity/{qid}")
}

fn smiles_page(records: &[(&str, &str, &str, &str)]) -> String {
    let bindings: Vec<serde_json::Value> = records
        .iter()
        .map(|(compound, inchikey, smiles, kind)| {
            serde_json::json!({
                "compound": { "value": format_compound(compound) },
                "inchiKey": { "value": inchikey },
                "smiles": { "value": smiles },
                "smilesKind": { "value": kind },
            })
        })
        .collect();
    serde_json::json!({
        "head": { "vars": ["compound", "inchiKey", "smiles", "smilesKind"] },
        "results": { "bindings": bindings }
    })
    .to_string()
}

fn smiles_without_inchikey_page(records: &[(&str, &str, &str)]) -> String {
    let bindings: Vec<serde_json::Value> = records
        .iter()
        .map(|(compound, smiles, kind)| {
            serde_json::json!({
                "compound": { "value": format_compound(compound) },
                "smiles": { "value": smiles },
                "smilesKind": { "value": kind },
            })
        })
        .collect();
    serde_json::json!({
        "head": { "vars": ["compound", "smiles", "smilesKind"] },
        "results": { "bindings": bindings }
    })
    .to_string()
}

#[tokio::test]
async fn run_archive_fetches_all_pages_and_deduplicates() {
    // ARCHIVE_PAGE_SIZE is 50_000, so any page shorter than that ends the loop.
    // The first page carries two records sharing InChIKey IK1: Q1 canonical and
    // Q3 isomeric. Isomeric wins on dedup.
    let http = Scripted::new(vec![
        json_reply(
            200,
            &smiles_page(&[
                ("Q1", "IK1", "CCO", "canonical"),
                ("Q3", "IK1", "OCC", "isomeric"), // same InChIKey, isomeric wins
                ("Q2", "IK2", "CC", "isomeric"),
            ]),
        ),
        json_reply(
            200,
            &smiles_without_inchikey_page(&[("Q4", "O", "canonical")]),
        ),
    ]);

    let (artifacts, summary) = run_archive(&http, endpoint(), "2026-03-04T05:06:07Z")
        .await
        .expect("the archive runs");

    assert_eq!(summary.source_records, 3);
    assert_eq!(summary.missing_inchi_key_records, 1);
    // IK1 appears twice: Q1 canonical CCO and Q3 isomeric OCC. Isomeric wins.
    assert_eq!(summary.records, 2);
    assert_eq!(summary.parse_errors, 0);

    // Verify the tar.gz contains the right members.
    let members = decode_archive(&artifacts.archive);

    let smiles_csv = members
        .get("lotus-wikidata-smiles.csv")
        .expect("smiles csv");
    assert!(smiles_csv.contains("IK1,Q3,isomeric,OCC"));
    assert!(smiles_csv.contains("IK2,Q2,isomeric,CC"));
    assert!(!smiles_csv.contains("Q1")); // Q1 was deduplicated away
    let missing_csv = members
        .get("missing-inchikey.csv")
        .expect("missing-inchikey csv");
    assert_eq!(
        missing_csv,
        "wikidata_id,smiles_kind,smiles\nQ4,canonical,O\n"
    );
    let errors_csv = members
        .get("smiles-parse-errors.csv")
        .expect("parse errors csv");
    assert_eq!(
        errors_csv,
        "inchi_key,wikidata_id,smiles_kind,smiles,error\n"
    );

    // Verify standalone manifest and checksums.
    let manifest: serde_json::Value =
        serde_json::from_str(&artifacts.manifest).expect("manifest JSON");
    assert_eq!(
        manifest
            .get("fetched_at")
            .and_then(serde_json::Value::as_str),
        Some("2026-03-04T05:06:07Z")
    );
    assert_eq!(
        manifest
            .get("source_records")
            .and_then(serde_json::Value::as_u64),
        Some(3)
    );
    assert_eq!(
        manifest.get("records").and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        manifest
            .get("missing_inchi_key_records")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        manifest
            .get("page_size")
            .and_then(serde_json::Value::as_u64),
        Some(ARCHIVE_PAGE_SIZE as u64)
    );

    let checksums = &artifacts.checksums;
    assert!(checksums.contains("  lotus-wikidata-smiles.tar.gz\n"));
    assert!(checksums.contains("  lotus-wikidata-smiles.manifest.json\n"));

    // Verify the manifest inside the archive matches the standalone one.
    let archive_manifest = members.get("manifest.json").expect("manifest in archive");
    assert_eq!(archive_manifest, &artifacts.manifest);
}

#[tokio::test]
async fn run_archive_handles_empty_results() {
    let http = Scripted::new(vec![
        json_reply(200, &smiles_page(&[])),
        json_reply(200, &smiles_without_inchikey_page(&[])),
    ]);

    let (artifacts, summary) = run_archive(&http, endpoint(), "2026-01-01T00:00:00Z")
        .await
        .expect("the archive runs");

    assert_eq!(summary.source_records, 0);
    assert_eq!(summary.records, 0);
    assert_eq!(summary.missing_inchi_key_records, 0);

    let manifest: serde_json::Value =
        serde_json::from_str(&artifacts.manifest).expect("manifest JSON");
    assert_eq!(
        manifest
            .get("source_records")
            .and_then(serde_json::Value::as_u64),
        Some(0)
    );
    assert_eq!(
        manifest.get("records").and_then(serde_json::Value::as_u64),
        Some(0)
    );
}

#[test]
fn deduplication_prefers_isomeric_over_canonical() {
    let selected = deduplicate_by_inchi_key(vec![
        SmilesRecord {
            inchi_key: "KEY1".to_string(),
            wikidata_id: "Q1".to_string(),
            smiles_kind: SmilesKind::Canonical,
            smiles: "CCO".to_string(),
        },
        SmilesRecord {
            inchi_key: "KEY1".to_string(),
            wikidata_id: "Q2".to_string(),
            smiles_kind: SmilesKind::Isomeric,
            smiles: "C[C@H](O)C".to_string(),
        },
        SmilesRecord {
            inchi_key: "KEY2".to_string(),
            wikidata_id: "Q3".to_string(),
            smiles_kind: SmilesKind::Canonical,
            smiles: "N".to_string(),
        },
    ]);

    assert_eq!(
        selected,
        vec![
            SmilesRecord {
                inchi_key: "KEY1".to_string(),
                wikidata_id: "Q2".to_string(),
                smiles_kind: SmilesKind::Isomeric,
                smiles: "C[C@H](O)C".to_string(),
            },
            SmilesRecord {
                inchi_key: "KEY2".to_string(),
                wikidata_id: "Q3".to_string(),
                smiles_kind: SmilesKind::Canonical,
                smiles: "N".to_string(),
            },
        ]
    );
}

#[test]
fn write_archive_produces_checksums_and_manifest() {
    let records = vec![SmilesRecord {
        inchi_key: "KEY1".to_string(),
        wikidata_id: "Q1".to_string(),
        smiles_kind: SmilesKind::Canonical,
        smiles: "CCO".to_string(),
    }];
    let missing = vec![MissingInchiKeyRecord {
        wikidata_id: "Q2".to_string(),
        smiles_kind: SmilesKind::Canonical,
        smiles: "O".to_string(),
    }];
    let errors = vec![];

    let artifacts = write_archive(
        "2026-09-17T00:00:00Z",
        DEFAULT_QLEVER_ENDPOINT,
        1,
        &missing,
        &records,
        &errors,
    )
    .expect("write_archive succeeds");

    let members = decode_archive(&artifacts.archive);

    let smiles_csv = members
        .get("lotus-wikidata-smiles.csv")
        .expect("smiles csv");
    assert!(smiles_csv.contains("KEY1,Q1,canonical,CCO"));
    let missing_csv = members
        .get("missing-inchikey.csv")
        .expect("missing-inchikey csv");
    assert_eq!(
        missing_csv,
        "wikidata_id,smiles_kind,smiles\nQ2,canonical,O\n"
    );
    let errors_csv = members
        .get("smiles-parse-errors.csv")
        .expect("parse errors csv");
    assert_eq!(
        errors_csv,
        "inchi_key,wikidata_id,smiles_kind,smiles,error\n"
    );
    let archive_manifest = members.get("manifest.json").expect("manifest in archive");
    assert_eq!(archive_manifest, &artifacts.manifest);

    let checksums = &artifacts.checksums;
    assert!(checksums.contains("  lotus-wikidata-smiles.tar.gz\n"));
    assert!(checksums.contains("  lotus-wikidata-smiles.manifest.json\n"));
}
