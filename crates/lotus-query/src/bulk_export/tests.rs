// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit tests for `bulk_export`.

use super::*;

#[test]
fn extract_qid_strips_the_entity_prefix() {
    assert_eq!(extract_qid("http://www.wikidata.org/entity/Q153"), "Q153");
    assert_eq!(extract_qid("Q153"), "Q153");
    assert_eq!(
        extract_qid("http://example.org/not-wikidata"),
        "http://example.org/not-wikidata"
    );
}

/// Every query's prefixes, spelled out here rather than taken from the module
/// so a change to the shared block shows up as a test failure.
const PREFIXES: &str = "\
PREFIX wdt: <http://www.wikidata.org/prop/direct/>
PREFIX p: <http://www.wikidata.org/prop/>
PREFIX ps: <http://www.wikidata.org/prop/statement/>
PREFIX prov: <http://www.w3.org/ns/prov#>
PREFIX pr: <http://www.wikidata.org/prop/reference/>
PREFIX wd: <http://www.wikidata.org/entity/>
";

/// Every query as `(query, text before the prefixes, text after them)`.
///
/// Only the compound labels put a prefix of their own ahead of the shared block.
const CASES: [(&str, &str, &str); 28] = [
    (
        COMPOUND_TAXON_REFERENCE_QUERY,
        "",
        r"
SELECT DISTINCT ?compound ?compound_inchikey ?taxon ?reference WHERE {
  ?compound wdt:P235 ?compound_inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
}",
    ),
    (
        EXPORT_COMPOUNDS_QUERY,
        "",
        r"
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
}",
    ),
    (
        EXPORT_TAXA_QUERY,
        "",
        r#"
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
  OPTIONAL { ?_taxon_rank rdfs:label ?taxon_rank_label . FILTER(LANG(?taxon_rank_label) = "en") }
}"#,
    ),
    (
        EXPORT_REFERENCES_QUERY,
        "",
        r"
SELECT ?reference ?doi ?pmid ?pmcid ?title ?date ?journal ?journal_label WHERE {
  {
    SELECT DISTINCT ?reference WHERE {
      {
        SELECT ?statement WHERE {
          ?compound wdt:P235 ?inchikey ;
                    p:P703 ?statement .
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
}",
    ),
    (
        COMPOUND_SMILES_CAN_QUERY,
        "",
        r"
SELECT DISTINCT ?compound ?compound_smiles_can WHERE {
  ?compound wdt:P235 ?inchikey ;
           wdt:P703 ?taxon ;
           wdt:P233 ?compound_smiles_can .
}",
    ),
    (
        COMPOUND_SMILES_ISO_QUERY,
        "",
        r"
SELECT DISTINCT ?compound ?compound_smiles_iso WHERE {
  ?compound wdt:P235 ?inchikey ;
           wdt:P703 ?taxon ;
           wdt:P2017 ?compound_smiles_iso .
}",
    ),
    (
        COMPOUND_CID_QUERY,
        "",
        r"
SELECT DISTINCT ?compound ?compound_cid WHERE {
  ?compound wdt:P235 ?inchikey ;
           wdt:P703 ?taxon ;
           wdt:P662 ?compound_cid .
}",
    ),
    (
        TAXON_NAME_QUERY,
        "",
        r"
SELECT DISTINCT ?taxon ?taxon_name WHERE {
  ?taxon wdt:P225 ?taxon_name .
}",
    ),
    (
        TAXON_NCBI_QUERY,
        "",
        r"
SELECT DISTINCT ?taxon ?taxon_ncbi WHERE {
  ?taxon wdt:P685 ?taxon_ncbi .
}",
    ),
    (
        TAXON_OTT_QUERY,
        "",
        r"
SELECT DISTINCT ?taxon ?taxon_ott WHERE {
  ?taxon wdt:P9157 ?taxon_ott .
}",
    ),
    (
        TAXON_GBIF_QUERY,
        "",
        r"
SELECT DISTINCT ?taxon ?taxon_gbif WHERE {
  ?taxon wdt:P846 ?taxon_gbif .
}",
    ),
    (
        TAXON_PARENT_QUERY,
        "",
        r"
SELECT DISTINCT ?taxon ?taxon_parent WHERE {
  ?taxon wdt:P171 ?taxon_parent .
  ?taxon_parent wdt:P171* wd:Q2382443 .
}",
    ),
    (
        TAXON_RANK_QUERY,
        "",
        r#"
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
SELECT DISTINCT ?taxon ?taxon_rank ?taxon_rank_label WHERE {
  ?taxon wdt:P105 ?taxon_rank .
  ?taxon_rank rdfs:label ?taxon_rank_label .
  FILTER(LANG(?taxon_rank_label) = "en")
}"#,
    ),
    (
        REFERENCE_DOI_QUERY,
        "",
        r"
SELECT DISTINCT ?reference ?doi WHERE {
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P356 ?doi .
}",
    ),
    (
        REFERENCE_PMID_QUERY,
        "",
        r"
SELECT DISTINCT ?reference ?pmid WHERE {
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P698 ?pmid .
}",
    ),
    (
        REFERENCE_PMCID_QUERY,
        "",
        r"
SELECT DISTINCT ?reference ?pmcid WHERE {
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P932 ?pmcid .
}",
    ),
    (
        REFERENCE_TITLE_QUERY,
        "",
        r"
SELECT DISTINCT ?reference ?title WHERE {
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P1476 ?title .
}",
    ),
    (
        REFERENCE_DATE_QUERY,
        "",
        r"
SELECT DISTINCT ?reference ?date WHERE {
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P577 ?date .
}",
    ),
    (
        REFERENCE_JOURNAL_QUERY,
        "",
        r"
SELECT DISTINCT ?reference ?journal ?journal_label WHERE {
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P1433 ?journal .
  ?journal wdt:P1476 ?journal_label .
}",
    ),
    (
        COMPOUND_INCHIKEY_TAXON_QUERY,
        "",
        r"
SELECT DISTINCT ?compound ?compound_inchikey ?taxon WHERE {
  ?compound wdt:P235 ?compound_inchikey ;
        wdt:P703 ?taxon .
}",
    ),
    (
        COMPOUND_PARENT_QUERY,
        "",
        r"
SELECT DISTINCT ?compound ?compound_parent WHERE {
  ?compound wdt:P279 ?compound_parent .
  ?compound_parent wdt:P279* wd:Q11173 .
}",
    ),
    (
        COMPOUND_SMILES_CAN_ALL_QUERY,
        "",
        r"
SELECT DISTINCT ?compound ?compound_smiles_can WHERE {
  ?compound wdt:P233 ?compound_smiles_can .
}",
    ),
    (
        COMPOUND_SMILES_ISO_ALL_QUERY,
        "",
        r"
SELECT DISTINCT ?compound ?compound_smiles_iso WHERE {
  ?compound wdt:P2017 ?compound_smiles_iso .
}",
    ),
    (
        COMPOUND_SMARTS_QUERY,
        "",
        r"
SELECT DISTINCT ?compound ?compound_smarts WHERE {
  ?compound wdt:P8533 ?compound_smarts .
}",
    ),
    (
        COMPOUND_CXSMILES_QUERY,
        "",
        r"
SELECT DISTINCT ?compound ?compound_cxsmiles WHERE {
  ?compound wdt:P10718 ?compound_cxsmiles .
}",
    ),
    (
        COMPOUND_LABEL_QUERY,
        "PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>\n",
        r#"
SELECT DISTINCT ?compound ?compound_label ?lang WHERE {
  ?compound wdt:P279* wd:Q11173 .
  ?compound rdfs:label ?compound_label .
  BIND(LANG(?compound_label) AS ?lang)
  FILTER (?lang IN ("en", "mul"))
}"#,
    ),
    (
        REFERENCE_DOI_COMPOUND_TAXON_QUERY,
        "",
        r"
SELECT DISTINCT ?compound ?taxon ?reference ?doi WHERE {
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P356 ?doi .
}",
    ),
    (
        REFERENCE_PMID_COMPOUND_TAXON_QUERY,
        "",
        r"
SELECT DISTINCT ?compound ?taxon ?reference ?pmid WHERE {
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P698 ?pmid .
}",
    ),
];

#[test]
fn every_bulk_export_query_is_exactly_the_transcribed_sparql() {
    for (actual, before, after) in CASES {
        assert_eq!(actual, [before, PREFIXES, after].concat());
    }
}

#[test]
fn archive_queries_are_paged_by_offset() {
    let page = archive_page(ARCHIVE_SMILES_QUERY, 0);
    assert!(page.contains("LIMIT 50000"));
    assert!(page.contains("OFFSET 0"));
    let page = archive_page(ARCHIVE_SMILES_QUERY, 50_000);
    assert!(page.contains("OFFSET 50000"));
    // The LIMIT is a constant, never derived from the offset argument.
    assert_eq!(page.matches("LIMIT").count(), 1);
}

#[test]
fn archive_inchikey_validation_rejects_non_inchikeys() {
    assert!(looks_like_archive_inchikey("LFQSCWFLJHTTHZ-UHFFFAOYSA-N"));
    assert!(!looks_like_archive_inchikey("not-an-inchikey"));
    assert!(!looks_like_archive_inchikey(""));
}
