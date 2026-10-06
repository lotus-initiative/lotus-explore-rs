// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `taxon_cache`, in their own file.

use super::*;

#[test]
fn lookup_returns_none_for_empty_key() {
    assert!(lookup("").is_none());
    assert!(lookup("   ").is_none());
}

fn resolved(qid: &str, label: &str) -> CachedTaxon {
    CachedTaxon {
        qid: qid.to_owned(),
        label: label.to_owned(),
        source: TaxonNameSource::Scientific,
        candidates: Vec::new(),
    }
}

#[test]
fn store_and_lookup_roundtrip() {
    store("Gentiana lutea", &resolved("Q2598745", "Gentiana lutea"));
    let result = lookup("gentiana lutea");
    assert_eq!(result.map(|cached| cached.qid), Some("Q2598745".into()));
}

#[test]
fn store_ignores_empty_qid() {
    store("Somespecies", &resolved("", "Somespecies"));
    assert!(lookup("somespecies").is_none());
}

#[test]
fn an_unambiguous_resolution_earns_no_notice() {
    assert_eq!(resolved("Q1", "Rosa").warnings().len(), 0);
}

#[test]
fn a_common_name_resolution_earns_the_common_name_notice_on_every_read() {
    let cached = CachedTaxon {
        qid: "Q777".to_owned(),
        label: "Gentian".to_owned(),
        source: TaxonNameSource::Common,
        candidates: Vec::new(),
    };
    store("gentian", &cached);

    let warnings = lookup("Gentian").map(|hit| hit.warnings());
    assert!(
        matches!(warnings, Some(ref w) if matches!(w[..], [LookupNotice::CommonName { .. }])),
        "expected the common-name notice, got {warnings:?}"
    );
}

#[test]
fn a_scientific_resolution_earns_nothing() {
    assert_eq!(resolved("Q1", "Gentiana lutea").warnings().len(), 0);
}

#[test]
fn a_common_name_that_is_also_ambiguous_earns_both_notices() {
    let cached = CachedTaxon {
        qid: "Q777".to_owned(),
        label: "Gentian".to_owned(),
        source: TaxonNameSource::Common,
        candidates: vec!["Gentian (Q777)".to_owned(), "Gentian (Q778)".to_owned()],
    };
    assert!(matches!(
        cached.warnings().as_slice(),
        [
            LookupNotice::CommonName { .. },
            LookupNotice::AmbiguousTaxon { .. }
        ]
    ));
}

#[test]
fn an_ambiguous_resolution_reproduces_the_same_notice_on_every_read() {
    let candidates = vec![
        "Bacteria (Q10876)".to_owned(),
        "Bacteria (Q4034791)".to_owned(),
    ];
    store(
        "bacteria",
        &CachedTaxon {
            qid: "Q10876".to_owned(),
            label: "Bacteria".to_owned(),
            source: TaxonNameSource::Scientific,
            candidates,
        },
    );

    let first = lookup("bacteria").map(|cached| cached.warnings());
    let second = lookup("Bacteria").map(|cached| cached.warnings());

    assert_eq!(
        first, second,
        "the notices must not depend on the cache path"
    );
    assert!(matches!(
        first.as_deref(),
        Some([LookupNotice::AmbiguousTaxon { .. }])
    ));
}
