// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! The rows the tests of every crate build, in one place.
//!
//! `full_row` and `empty_row` used to be spelled out as identical struct
//! literals in `lotus-model`'s own integration tests, in `lotus-query`'s unit
//! tests and in `lotus-query`'s export-fidelity test. Two spellings of one row
//! is the thing `cargo dejadoc --context-blocks` reports: a reader editing one
//! has no way to know the other exists, and nothing stops the two drifting into
//! fixtures that no longer test the same thing.
//!
//! Behind the `testing` feature so it stays out of every consumer's build, and
//! turned on through a dev-dependency -- the same arrangement `lotus-search`
//! uses for its scripted transport. A fixture is not part of a consumer's API.

use crate::CompoundEntry;
use std::sync::Arc;

fn arc(text: &str) -> Arc<str> {
    Arc::from(text)
}

/// One row with every optional column populated, so no dictionary is empty and
/// no exporter branch is skipped.
///
/// The values are awkward on purpose where awkwardness is a branch: a comma in
/// the DOI, a hex reference node, a statement to reify. Tests that need one
/// field different override it with `..full_row()`.
#[must_use]
pub fn full_row() -> CompoundEntry {
    CompoundEntry {
        compound_qid: arc("Q3613679"),
        name: arc("quercetin"),
        inchikey: Some(arc("ABCDEF-GHIJKL-M")),
        smiles: Some(arc("O=c1cc(-c2ccccc2)oc2cc(O)cc(O)c12")),
        mass: Some(302.24),
        formula: Some(arc("C15H10O7")),
        taxon_qid: arc("Q128267"),
        taxon_name: arc("Rosa"),
        reference_qid: arc("Q100000001"),
        reference_node: arc("6eff7d028afee42232e3963a2c0f9d3b7e5a41c8d2f60b9e3a7d5c1f8b2e6049a"),
        ref_title: Some(arc("Flavonoid isolation, 1971")),
        ref_doi: Some(arc("10.1000/a, b")),
        pub_year: Some(1971),
        statement: Some(arc("Q200000002")),
    }
}

/// One row with every optional column absent, so the empty-cell paths of every
/// reader and exporter are exercised at once: short cells, empty strings, an
/// absent year.
#[must_use]
pub fn empty_row() -> CompoundEntry {
    CompoundEntry {
        compound_qid: arc("Q42"),
        name: arc(""),
        inchikey: None,
        smiles: None,
        mass: None,
        formula: None,
        taxon_qid: arc(""),
        taxon_name: arc(""),
        reference_qid: arc(""),
        reference_node: arc(""),
        ref_title: None,
        ref_doi: None,
        pub_year: None,
        statement: None,
    }
}
