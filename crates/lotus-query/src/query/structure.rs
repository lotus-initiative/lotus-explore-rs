// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Chemical structure SPARQL builders delegating to IDSM/Sachem.

use lotus_model::SmilesSearchType;

use super::Nomenclature;
use super::nomenclature::taxon_ancestry_pattern;
use super::prefixes::{
    COMPOUND_PROPERTIES, PREFIXES_WITH_STRUCTURE, REFERENCE_METADATA, SELECT_CLAUSE,
};
use super::rewrite::escape_structure_literal;

/// A structure search through the IDSM/Sachem service.
///
/// The route for [`SmilesSearchType::Substructure`] and [`SmilesSearchType::Similarity`],
/// and for [`SmilesSearchType::Exact`] when the input resolved to no compound at all. Exact
/// otherwise has the cheaper [`exact_compound_query`](super::exact_compound_query), used whenever a QID is available.
///
/// Without `taxon_qid` the occurrence triple is optional, so a compound with no occurrence
/// data is still returned -- structure search is how such compounds are found at all.
/// With `Some("*")` occurrences are required across any taxon without expanding the Biota
/// ancestry closure.
#[must_use]
pub fn structure_search_query(
    structure: &str,
    search: SmilesSearchType,
    threshold: f64,
    taxon_qid: Option<&str>,
) -> String {
    structure_search_query_with(
        structure,
        search,
        threshold,
        taxon_qid,
        &Nomenclature::ALL_ON,
    )
}

fn idsm_service_clause(structure: &str, search: SmilesSearchType, threshold: f64) -> String {
    let literal = escape_structure_literal(structure);
    let is_multiline = literal.starts_with("'''");

    // A structure only reaches this function when it did *not* resolve to a
    // Wikidata compound; an input that named one is answered by
    // `exact_compound_query` without a service at all. Nothing that reaches here
    // has an identity to search for, so the substructure service is the only thing
    // that can answer. Documented rather than silent: see
    // `docs/STRUCTURE-SEARCH.md`.
    match search {
        // `Exact` is answered by `exact_compound_query` before it gets here. It
        // reaches this function only when the input named nothing, so the closest
        // answer the service can give is the same molecule: an identical
        // fingerprint, which is a similarity search at a cutoff of 1.
        SmilesSearchType::Exact => format!(
            "SERVICE idsm:wikidata {{
    ?c sachem:similarCompoundSearch [
      sachem:query {literal};
      sachem:cutoff \"1\"^^xsd:double
    ].
  }}"
        ),
        SmilesSearchType::Substructure if is_multiline => format!(
            "SERVICE idsm:wikidata {{
    [ sachem:compound ?c; sachem:score ?_sachem_score ]
      sachem:scoredSubstructureSearch [
        sachem:query {literal};
        sachem:searchMode sachem:substructureSearch;
        sachem:chargeMode sachem:defaultChargeAsAny;
        sachem:isotopeMode sachem:ignoreIsotopes;
        sachem:aromaticityMode sachem:aromaticityDetectIfMissing;
        sachem:stereoMode sachem:ignoreStereo;
        sachem:tautomerMode sachem:ignoreTautomers;
        sachem:radicalMode sachem:ignoreSpinMultiplicity;
        sachem:topn \"-1\"^^xsd:integer;
        sachem:internalMatchingLimit \"1000000\"^^xsd:integer
      ].
  }}"
        ),
        SmilesSearchType::Similarity => format!(
            "SERVICE idsm:wikidata {{
    ?c sachem:similarCompoundSearch [
      sachem:query {literal};
      sachem:cutoff \"{threshold}\"^^xsd:double
    ].
  }}"
        ),
        SmilesSearchType::Substructure => format!(
            "SERVICE idsm:wikidata {{
    ?c sachem:substructureSearch [
      sachem:query {literal}
    ].
  }}"
        ),
    }
}

/// [`compounds_by_taxon_query_with`](super::compounds_by_taxon_query_with): a structure
/// search is run *within* a taxon, and the taxon's nomenclatural closure is the same set
/// of organisms whether the compound was found by name or by substructure.
#[must_use]
pub fn structure_search_query_with(
    structure: &str,
    search: SmilesSearchType,
    threshold: f64,
    taxon_qid: Option<&str>,
    nomenclature: &Nomenclature,
) -> String {
    let service = idsm_service_clause(structure, search, threshold);

    // With a taxon (or `"*"`, meaning any taxon with a recorded occurrence),
    // the service runs in its own subquery so that `QLever` can pre-filter
    // before enriching, and the occurrence triple is *required* — a match
    // outside the requested scope should not come back. For `"*"` no `P171*`
    // Biota closure is emitted, since `ps:P703` already constrains to recorded
    // taxon occurrences directly (~2.7s vs ~16s+ on indole substructure).
    //
    // Without one, the service joins the body directly and the occurrence
    // triple is `OPTIONAL`: a structure search is how a compound with no
    // occurrence data gets found in the first place, and requiring a taxon
    // would hide exactly those. Repeating `{service}` inside the `OPTIONAL`
    // filters the `P703` occurrence scan by the matched compounds before
    // joining `prov:wasDerivedFrom` and `pr:P248` (~630 ms vs >18s on `c1ccccc1`).
    let body = taxon_qid.map_or_else(
        || {
            format!(
                "
  {service}

  ?c wdt:P235 ?compound_inchikey .

  OPTIONAL {{
    ?c p:P703 ?statement_id .
    {service}
    ?statement_id ps:P703 ?t ;
                  wikibase:rank ?_stmt_rank ;
                  prov:wasDerivedFrom ?ref_node .
    FILTER(?_stmt_rank != wikibase:DeprecatedRank)
    ?ref_node pr:P248 ?r .
    ?t wdt:P225 ?taxon_name .
  }}

  {REFERENCE_METADATA}
  {COMPOUND_PROPERTIES}
"
            )
        },
        |qid| {
            let taxon_filter = if qid == "*" {
                String::new()
            } else {
                format!(
                    "\n  {{\n    SELECT DISTINCT ?t\n    WHERE {{\n      {}\n    }}\n  }}",
                    taxon_ancestry_pattern(qid, *nomenclature)
                )
            };
            format!(
                "
  {{
    SELECT DISTINCT ?c
    WHERE {{
      {service}
    }}
  }}

  ?c wdt:P235 ?compound_inchikey .
  ?c p:P703 ?statement_id .
  ?statement_id ps:P703 ?t ;
                wikibase:rank ?_stmt_rank ;
                prov:wasDerivedFrom ?ref_node .
  FILTER(?_stmt_rank != wikibase:DeprecatedRank)
  ?ref_node pr:P248 ?r .
  ?t wdt:P225 ?taxon_name .{taxon_filter}

  {REFERENCE_METADATA}
  {COMPOUND_PROPERTIES}
"
            )
        },
    );

    format!(
        "{PREFIXES_WITH_STRUCTURE}
{SELECT_CLAUSE}
WHERE {{
{body}
}}"
    )
}
