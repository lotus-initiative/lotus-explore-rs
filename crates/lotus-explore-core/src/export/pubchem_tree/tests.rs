// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit tests for `pubchem_tree`.

use super::*;

fn row_map(pairs: [(&str, &str); 3]) -> Row {
    let mut m = Row::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v.to_string());
    }
    m
}

#[test]
fn build_compounds_with_taxa_filters_without_inchikey() {
    let data = PubChemData {
        compound_taxon: vec![
            row_map([
                ("compound", "http://www.wikidata.org/entity/Q1"),
                ("compound_inchikey", "IK1"),
                ("taxon", "http://www.wikidata.org/entity/Q2"),
            ]),
            row_map([
                ("compound", "http://www.wikidata.org/entity/Q3"),
                ("compound_inchikey", ""),
                ("taxon", "http://www.wikidata.org/entity/Q4"),
            ]),
        ],
        ..Default::default()
    };
    let (set, map) = build_compounds_with_taxa(&data);
    assert!(set.contains("Q1"));
    assert!(!set.contains("Q3"));
    assert_eq!(map.get("Q1"), Some(&vec!["IK1".to_string()]));
}

#[test]
fn simplify_returns_single_value_when_unique() {
    assert_eq!(simplify(&["a".to_string()]), Value::String("a".to_string()));
    assert_eq!(
        simplify(&["a".to_string(), "b".to_string()]),
        Value::Array(vec![
            Value::String("a".to_string()),
            Value::String("b".to_string()),
        ])
    );
}

#[test]
fn count_tree_nodes_counts_recursively() {
    let tree = vec![node("A", "Q1"), node("B", "Q2")];
    assert_eq!(count_tree_nodes(&tree), 2);
}
