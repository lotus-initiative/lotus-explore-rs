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
            "compound_item": {"value": "http://www.wikidata.org/entity/Q20"},
            "canonical": {"value": "CCO"}
        },
        {
            "key": {"value": "K"},
            "compound_item": {"value": "http://www.wikidata.org/entity/Q100"}
        },
        {
            "key": {"value": "K"},
            "compound_item": {"value": "http://www.wikidata.org/entity/Q10"}
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
