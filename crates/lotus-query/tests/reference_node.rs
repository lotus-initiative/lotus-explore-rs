// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The reference node, as the streaming reader normalises it.
//!
//! `ref_node` is the one column that arrives as a full Wikidata URI while
//! every other identifier column arrives bare. The export writes the remainder
//! -- the 64-hex node hash -- because that is what identifies the reference, and
//! a URI in a file column is a link that breaks the day the base URL moves.
//!
//! Mutation testing found this: replacing `normalize_reference_node` with `None`
//! and with `Some("")` both survived, because every fixture in the tree left the
// column empty.

// The panic lints keep library code free of panics on external input. A test
// failing on a bad fixture is reporting, not panicking.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus_model::WIKIDATA_REFERENCE_BASE;
use lotus_query::parse_compounds_columnar;

/// The header the endpoint answers a results query with, in the order it sends
/// it. `ref_node` last, which is where it is in the real projection too.
const HEADER: &str = "compound,compoundLabel,compound_inchikey,compound_smiles_conn,\
                       compound_smiles_iso,compound_mass,compound_formula,taxon,taxon_name,\
                       ref_qid,ref_title,ref_doi,ref_date,ref_node\n";

const HASH: &str = "9F1C0E4A-7B2D-4C6E-8A91-3D5F7B2E9C10";

/// One row, with `node` as the `ref_node` cell.
fn payload(node: &str) -> String {
    format!(
        "{HEADER}Q1,Quercetin,IK1,CCO,,180.16,C9H8O4,Q16521,Gentiana lutea,Q100,T,10.1/a,2021,{node}\n"
    )
}

/// Parse `body` and return row 0's `reference_node`.
fn reference_node_of(body: &str) -> String {
    let set = parse_compounds_columnar(body.as_bytes()).expect("the CSV parses");
    assert_eq!(set.row_count(), 1, "one row in, one row out");
    set.entry(0)
        .expect("row 0 is present")
        .reference_node
        .to_string()
}

#[test]
fn a_reference_node_arrives_as_the_bare_hash() {
    // The whole reason the normalisation exists. A URI here would also be the
    // one column in the file that is not comparable to the others.
    let node = reference_node_of(&payload(&format!("{WIKIDATA_REFERENCE_BASE}{HASH}")));
    assert_eq!(
        node, HASH,
        "the base must be stripped: a URI in an identifier column breaks the \
         day the base URL moves, and it is not comparable to ref_qid"
    );
    assert!(
        !node.contains("http"),
        "a scheme survived into the identifier: {node:?}"
    );
}

#[test]
fn a_reference_node_that_is_already_bare_is_left_alone() {
    // Not every service sends the full URI, and a node that arrives bare must
    // not be mangled by a normalisation that expects a prefix.
    assert_eq!(reference_node_of(&payload(HASH)), HASH);
}

#[test]
fn an_empty_reference_node_is_empty_and_not_the_placeholder() {
    // A row with no reference node is a real case -- the taxon was resolved but
    // nothing cited it -- and the export has to emit an empty cell rather than
    // the string `None` or the base on its own.
    assert!(
        reference_node_of(&payload("")).is_empty(),
        "an absent reference node must not become a value"
    );
}

#[test]
fn a_whitespace_only_reference_node_is_also_empty() {
    // The cells arrive trimmed everywhere else; this one arrives from a
    // projection the reader does not otherwise touch, so a stray space would
    // otherwise survive as a one-character identifier.
    assert!(
        reference_node_of(&payload("   ")).is_empty(),
        "whitespace is not an identifier"
    );
}
