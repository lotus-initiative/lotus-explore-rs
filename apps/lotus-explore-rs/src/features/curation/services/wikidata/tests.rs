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
