// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Reference DOI (`P356`) single and batched lookup queries.

use super::statements::{escape_sparql_string, property};
use crate::CURATION_SPARQL_PREFIXES;

/// The item for a reference DOI, or `None`.
#[must_use]
pub fn reference_by_doi_query(doi: &str) -> String {
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?ref WHERE {{\n  \
           ?ref wdt:{doi_prop} \"{value}\" .\n\
         }} ORDER BY ?ref LIMIT 1",
        doi_prop = property::DOI,
        value = escape_sparql_string(doi),
    )
}

/// The batched reference lookup query for normalized DOIs.
#[must_use]
pub fn build_reference_lookup_query(dois: &[String]) -> String {
    let values = dois
        .iter()
        .map(|doi| {
            let escaped = escape_sparql_string(doi);
            format!("(\"{escaped}\" \"{escaped}\")")
        })
        .collect::<Vec<_>>()
        .join("\n    ");
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?lookup ?ref WHERE {{\n  \
           VALUES (?lookup ?doi) {{\n    \
             {values}\n  \
           }}\n  \
           ?ref wdt:P356 ?doi .\n\
         }}"
    )
}
