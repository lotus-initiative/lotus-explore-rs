// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `wikidata`, in their own file.

#![allow(clippy::expect_used)]

use super::*;

#[test]
fn qid_of_extracts_entity_id_from_wikidata_uri() {
    assert_eq!(
        qid_of("http://www.wikidata.org/entity/Q16521"),
        Some("Q16521".to_string())
    );
    assert_eq!(qid_of("not-a-uri"), None);
}

#[test]
fn resolve_or_create_taxon_returns_pre_resolved_qid_without_generating_statements() {
    let (qid, statements) =
        futures::executor::block_on(resolve_or_create_taxon("Gentiana lutea", Some("Q158559")))
            .expect("pre-resolved taxon");
    assert_eq!(qid.as_deref(), Some("Q158559"));
    assert_eq!(statements, Vec::<String>::new());
}

/// Two items share one `InChIKey`. The lower QID is kept by number, not by
/// string, and with its own properties rather than the other item's.
#[test]
fn a_shared_key_keeps_the_lower_item_with_its_own_properties() {
    let json = serde_json::json!({"results": {"bindings": [
        {
            "key": {"value": "K"},
            "compound": {"value": "http://www.wikidata.org/entity/Q20"},
            "canonical": {"value": "CCO"}
        },
        {
            "key": {"value": "K"},
            "compound": {"value": "http://www.wikidata.org/entity/Q100"}
        },
        {
            "key": {"value": "K"},
            "compound": {"value": "http://www.wikidata.org/entity/Q10"}
        }
    ]}});

    let resolved = compounds_by_key(&json);
    let kept = resolved.get("K");

    assert_eq!(kept.map(|c| c.qid.as_str()), Some("Q10"), "{kept:?}");
    assert_eq!(
        kept.and_then(|c| c.canonical_smiles.as_deref()),
        None,
        "Q10 states no canonical SMILES; Q20's must not be read as its"
    );
}

#[test]
fn parse_ask_boolean_requires_valid_boolean_field() {
    assert!(parse_ask_boolean(&serde_json::json!({"boolean": true})).expect("valid true"));
    assert!(!parse_ask_boolean(&serde_json::json!({"boolean": false})).expect("valid false"));
    assert!(
        parse_ask_boolean(&serde_json::json!({})).is_err(),
        "missing boolean field must error instead of defaulting to false"
    );
    assert!(
        parse_ask_boolean(&serde_json::json!({"boolean": "true"})).is_err(),
        "string boolean field must error instead of defaulting to false"
    );
}

#[test]
fn taxon_and_reference_ties_keep_lowest_qid_number_regardless_of_row_order() {
    let taxon_json = serde_json::json!({"results": {"bindings": [
        {"lookup": {"value": "gentiana"}, "taxon": {"value": "http://www.wikidata.org/entity/Q100"}},
        {"lookup": {"value": "gentiana"}, "taxon": {"value": "http://www.wikidata.org/entity/Q20"}},
        {"lookup": {"value": "gentiana"}, "taxon": {"value": "http://www.wikidata.org/entity/Q999"}}
    ]}});
    assert_eq!(
        taxon_qids_from_json(&taxon_json)
            .get("gentiana")
            .map(String::as_str),
        Some("Q20")
    );
    assert_eq!(
        extract_lowest_qid_from_json(&taxon_json, "taxon").as_deref(),
        Some("Q20")
    );

    let ref_json = serde_json::json!({"results": {"bindings": [
        {"lookup": {"value": "10.1/x"}, "ref": {"value": "http://www.wikidata.org/entity/Q200"}},
        {"lookup": {"value": "10.1/x"}, "ref": {"value": "http://www.wikidata.org/entity/Q30"}}
    ]}});
    assert_eq!(
        reference_qids_from_json(&ref_json)
            .get("10.1/x")
            .map(String::as_str),
        Some("Q30")
    );
}
