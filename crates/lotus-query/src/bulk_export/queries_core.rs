// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Core LOTUS exporter SPARQL queries (`lotus_exporter.py`).

/// The core triplet: compound-taxon-reference statements that carry `InChIKeys`.
///
/// Every compound that has an occurrence statement and an `InChIKey`, with the
/// taxon, reference, and reference metadata (title, DOI, date).
pub const COMPOUND_TAXON_REFERENCE_QUERY: &str = select_distinct!(
    "?compound ?compound_inchikey ?taxon ?reference",
    "  ?compound wdt:P235 ?compound_inchikey .",
    "  ?compound p:P703 ?statement .",
    "  ?statement ps:P703 ?taxon .",
    "  ?statement wikibase:rank ?_rank .",
    "  FILTER(?_rank != wikibase:DeprecatedRank)",
    "  ?statement prov:wasDerivedFrom/pr:P248 ?reference .",
);

/// Taxon scientific name (P225).
pub const TAXON_NAME_QUERY: &str = direct_property!("taxon", "P225", "taxon_name");

/// Taxon NCBI taxonomy ID (P685).
pub const TAXON_NCBI_QUERY: &str = direct_property!("taxon", "P685", "taxon_ncbi");

/// Taxon parent (P171), constrained to taxa under Biota (Q2382443).
pub const TAXON_PARENT_QUERY: &str = select_distinct!(
    "?taxon ?taxon_parent",
    "  ?taxon wdt:P171 ?taxon_parent .",
    "  ?taxon_parent wdt:P171* wd:Q2382443 .",
);

/// Consolidated compound properties (`SMILES`, isomeric `SMILES`, `PubChem` CID)
/// for every compound that carries an `InChIKey` and a taxon occurrence.
///
/// Deduplicating `?compound` in an inner subquery before joining `OPTIONAL`
/// structure properties fetches all three properties in one request without
/// repeating the occurrence scan three times.
pub const EXPORT_COMPOUNDS_QUERY: &str = concat!(
    prefixes!(),
    "
SELECT ?compound ?compound_smiles_can ?compound_smiles_iso ?compound_cid WHERE {
  {
    SELECT DISTINCT ?compound WHERE {
      ?compound wdt:P235 ?inchikey ;
                wdt:P703 ?taxon .
    }
  }
  OPTIONAL { ?compound wdt:P233 ?compound_smiles_can . }
  OPTIONAL { ?compound wdt:P2017 ?compound_smiles_iso . }
  OPTIONAL { ?compound wdt:P662 ?compound_cid . }
}"
);

/// Consolidated taxon properties (scientific name, NCBI/OTT/GBIF IDs, parent
/// taxon, and English rank label) scoped to taxa that carry a compound occurrence.
///
/// Scoping to occurring taxa avoids transferring all four million taxa in
/// Wikidata when only ~38,000 appear in LOTUS, and the `wd:Q0` sentinel keeps
/// an unbound `?taxon_rank` from enumerating every English label in the graph.
pub const EXPORT_TAXA_QUERY: &str = concat!(
    prefixes!(),
    "
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
SELECT ?taxon ?taxon_name ?taxon_ncbi ?taxon_ott ?taxon_gbif ?taxon_parent ?taxon_rank ?taxon_rank_label WHERE {
  {
    SELECT DISTINCT ?taxon WHERE {
      ?compound wdt:P235 ?inchikey ;
                wdt:P703 ?taxon .
    }
  }
  OPTIONAL { ?taxon wdt:P225 ?taxon_name . }
  OPTIONAL { ?taxon wdt:P685 ?taxon_ncbi . }
  OPTIONAL { ?taxon wdt:P9157 ?taxon_ott . }
  OPTIONAL { ?taxon wdt:P846 ?taxon_gbif . }
  OPTIONAL { ?taxon wdt:P171 ?taxon_parent . }
  OPTIONAL { ?taxon wdt:P105 ?taxon_rank . }
  BIND(COALESCE(?taxon_rank, wd:Q0) AS ?_taxon_rank)
  OPTIONAL { ?_taxon_rank rdfs:label ?taxon_rank_label . FILTER(LANG(?taxon_rank_label) = \"en\") }
}"
);

/// Consolidated bibliographic properties (DOI, PMID, PMCID, title, date,
/// journal and journal title) for references cited by occurrence statements.
///
/// Materializing the occurrence `?statement` set in an inner subquery before
/// traversing `prov:wasDerivedFrom/pr:P248` keeps `QLever` from scanning all
/// 130 million provenance triples in Wikidata, and the `wd:Q0` sentinel keeps
/// references without a journal (`P1433`) from scanning every titled item in
/// the graph.
pub const EXPORT_REFERENCES_QUERY: &str = concat!(
    prefixes!(),
    "
SELECT ?reference ?doi ?pmid ?pmcid ?title ?date ?journal ?journal_label WHERE {
  {
    SELECT DISTINCT ?reference WHERE {
      {
        SELECT ?statement WHERE {
          ?compound wdt:P235 ?inchikey ;
                    p:P703 ?statement .
          ?statement wikibase:rank ?_rank .
          FILTER(?_rank != wikibase:DeprecatedRank)
        }
      }
      ?statement ps:P703 ?taxon ;
                 prov:wasDerivedFrom/pr:P248 ?reference .
    }
  }
  OPTIONAL { ?reference wdt:P356 ?doi . }
  OPTIONAL { ?reference wdt:P698 ?pmid . }
  OPTIONAL { ?reference wdt:P932 ?pmcid . }
  OPTIONAL { ?reference wdt:P1476 ?title . }
  OPTIONAL { ?reference wdt:P577 ?date . }
  OPTIONAL { ?reference wdt:P1433 ?journal . }
  BIND(COALESCE(?journal, wd:Q0) AS ?_journal)
  OPTIONAL { ?_journal wdt:P1476 ?journal_label . }
}"
);
