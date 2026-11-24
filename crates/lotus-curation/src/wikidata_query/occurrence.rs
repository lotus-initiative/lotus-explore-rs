// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `P703` occurrence `ASK` and batched lookup queries.

use super::statements::property;
use crate::CURATION_SPARQL_PREFIXES;

/// Whether a compound already has an occurrence in a taxon.
#[must_use]
pub fn has_occurrence_query(compound_qid: &str, taxon_qid: &str) -> String {
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         ASK {{ wd:{compound} wdt:{occurs} wd:{taxon} }}",
        compound = compound_qid,
        occurs = property::OCCURS_IN_TAXON,
        taxon = taxon_qid,
    )
}

/// Whether a compound already has an occurrence in a taxon stated in a specific
/// reference (`P248` on the `P703` statement's reference node), excluding
/// `wikibase:DeprecatedRank` statements.
#[must_use]
pub fn has_occurrence_with_ref_query(compound_qid: &str, taxon_qid: &str, ref_qid: &str) -> String {
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         ASK {{\n  \
           wd:{compound} p:{occurs} ?stmt .\n  \
           ?stmt ps:{occurs} wd:{taxon} ;\n        \
                 wikibase:rank ?rank ;\n        \
                 prov:wasDerivedFrom ?refnode .\n  \
           ?refnode pr:P248 wd:{reference} .\n  \
           FILTER(?rank != wikibase:DeprecatedRank)\n\
         }}",
        compound = compound_qid,
        occurs = property::OCCURS_IN_TAXON,
        taxon = taxon_qid,
        reference = ref_qid,
    )
}

/// The batched `(compound, taxon)` occurrence query.
#[must_use]
pub fn build_occurrence_query(pairs: &[(String, String)]) -> String {
    let values = pairs
        .iter()
        .map(|(compound, taxon)| format!("(wd:{compound} wd:{taxon})"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT DISTINCT ?compound ?taxon WHERE {{\n  \
           VALUES (?compound ?taxon) {{ {values} }}\n  \
           ?compound wdt:{occurs} ?taxon .\n\
         }}",
        occurs = property::OCCURS_IN_TAXON,
    )
}

/// The batched `(compound, taxon, reference)` occurrence query.
#[must_use]
pub fn build_occurrence_with_ref_query(triples: &[(String, String, String)]) -> String {
    let values = triples
        .iter()
        .map(|(compound, taxon, reference)| format!("(wd:{compound} wd:{taxon} wd:{reference})"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{CURATION_SPARQL_PREFIXES}\n\
         SELECT DISTINCT ?compound ?taxon ?ref WHERE {{\n  \
           VALUES (?compound ?taxon ?ref) {{ {values} }}\n  \
           ?compound p:{occurs} ?stmt .\n  \
           ?stmt ps:{occurs} ?taxon ;\n        \
                 wikibase:rank ?rank ;\n        \
                 prov:wasDerivedFrom ?refnode .\n  \
           ?refnode pr:P248 ?ref .\n  \
           FILTER(?rank != wikibase:DeprecatedRank)\n\
         }}",
        occurs = property::OCCURS_IN_TAXON,
    )
}
