// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit tests for `endpoints`.

use super::*;

#[test]
fn endpoints_display_and_describe_exact_metadata() {
    assert_eq!(SparqlEndpoint::Qlever.to_string(), "QLever");
    assert_eq!(SparqlEndpoint::Wdqs.to_string(), "Wikidata Query Service");

    assert_eq!(
        SparqlEndpoint::Qlever.info(),
        (
            QLEVER_WIKIDATA,
            "QLever Wikidata",
            "Fast SPARQL endpoint for Wikidata (QLever)",
        )
    );
    assert_eq!(
        SparqlEndpoint::Wdqs.info(),
        (
            WDQS_WIKIDATA,
            "Wikidata Query Service",
            "Wikidata Query Service (QLever fallback)",
        )
    );
}
