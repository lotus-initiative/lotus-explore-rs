// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Compound lookup queries keyed by `InChIKey` (`P235`).

use super::statements::{escape_sparql_string, property};
use crate::CURATION_SPARQL_PREFIXES;

/// The item for a structure, matched on its `InChIKey`.
#[must_use]
pub fn compound_by_inchikey_query(inchikey: &str) -> String {
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?compound ?canonical ?iso ?inchi ?formula ?mass WHERE {{\n  \
           ?compound wdt:{inchikey_prop} \"{value}\" .\n  \
           OPTIONAL {{ ?compound wdt:{canonical} ?canonical . }}\n  \
           OPTIONAL {{ ?compound wdt:{iso} ?iso . }}\n  \
           OPTIONAL {{ ?compound wdt:{inchi} ?inchi . }}\n  \
           OPTIONAL {{ ?compound wdt:{formula} ?formula . }}\n  \
           OPTIONAL {{ ?compound wdt:{mass} ?mass . }}\n\
         }} ORDER BY STRLEN(STR(?compound)) ?compound LIMIT 1",
        inchikey_prop = property::INCHIKEY,
        value = escape_sparql_string(inchikey),
        canonical = property::CANONICAL_SMILES,
        iso = property::ISOMERIC_SMILES,
        inchi = property::INCHI,
        formula = property::FORMULA,
        mass = property::EXACT_MASS,
    )
}

/// Fetch every compound in a run with **one** query, keyed by `InChIKey`.
#[must_use]
pub fn build_compound_lookup_query(keys: &[String]) -> String {
    let values = keys
        .iter()
        .map(|key| format!("\"{}\"", escape_sparql_string(key)))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT ?key ?compound ?canonical ?iso ?inchi ?formula ?mass WHERE {{\n  \
           VALUES ?key {{ {values} }}\n  \
           ?compound wdt:P235 ?key .\n  \
           OPTIONAL {{ ?compound wdt:P233 ?canonical . }}\n  \
           OPTIONAL {{ ?compound wdt:P2017 ?iso . }}\n  \
           OPTIONAL {{ ?compound wdt:P234 ?inchi . }}\n  \
           OPTIONAL {{ ?compound wdt:P274 ?formula . }}\n  \
           OPTIONAL {{ ?compound wdt:P2067 ?mass . }}\n\
         }}"
    )
}
