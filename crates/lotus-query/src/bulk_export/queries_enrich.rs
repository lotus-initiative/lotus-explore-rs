// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `PubChem` tree (`lotus_pubchem_tree.py`) and Wikidata archive (`lotus-wikidata-archive`) SPARQL queries.

/// Compound-InChIKey-taxon triplets, the leaf compounds of the biological tree.
pub const COMPOUND_INCHIKEY_TAXON_QUERY: &str = select_distinct!(
    "?compound ?compound_inchikey ?taxon",
    "  ?compound wdt:P235 ?compound_inchikey ;",
    "        wdt:P703 ?taxon .",
);

/// Compound-parent (P279) pairs, constrained to compounds under chemical compound (Q11173).
pub const COMPOUND_PARENT_QUERY: &str = select_distinct!(
    "?compound ?compound_parent",
    "  ?compound wdt:P279 ?compound_parent .",
    "  ?compound_parent wdt:P279* wd:Q11173 .",
);

/// Compound canonical SMILES (P233).
pub const COMPOUND_SMILES_CAN_ALL_QUERY: &str =
    direct_property!("compound", "P233", "compound_smiles_can");

/// Compound isomeric SMILES (P2017).
pub const COMPOUND_SMILES_ISO_ALL_QUERY: &str =
    direct_property!("compound", "P2017", "compound_smiles_iso");

/// Compound SMARTS (P8533).
pub const COMPOUND_SMARTS_QUERY: &str = direct_property!("compound", "P8533", "compound_smarts");

/// Compound CXSMILES (P10718).
pub const COMPOUND_CXSMILES_QUERY: &str =
    direct_property!("compound", "P10718", "compound_cxsmiles");

/// Compound labels (rdfs:label), English or mul.
pub const COMPOUND_LABEL_QUERY: &str = concat!(
    "PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>\n",
    prefixes!(),
    "
SELECT DISTINCT ?compound ?compound_label ?lang WHERE {
  ?compound wdt:P279* wd:Q11173 .
  ?compound rdfs:label ?compound_label .
  BIND(LANG(?compound_label) AS ?lang)
  FILTER (?lang IN (\"en\", \"mul\"))
}"
);

/// Reference DOI with compound and taxon, for the biological tree's reference edges.
pub const REFERENCE_DOI_COMPOUND_TAXON_QUERY: &str = occurrence_reference!(
    "?compound ?taxon ?reference ?doi",
    "  ?reference wdt:P356 ?doi .",
);

/// Reference PMID with compound and taxon, for the biological tree's reference edges.
pub const REFERENCE_PMID_COMPOUND_TAXON_QUERY: &str = occurrence_reference!(
    "?compound ?taxon ?reference ?pmid",
    "  ?reference wdt:P698 ?pmid .",
);

/// Every compound with P703 (found in taxon) and P235 (`InChIKey`), with its
/// P233 canonical and/or P2017 isomeric SMILES. Paged with LIMIT/OFFSET by the
/// caller.
pub const ARCHIVE_SMILES_QUERY: &str = r#"
PREFIX wdt: <http://www.wikidata.org/prop/direct/>

SELECT DISTINCT ?compound ?inchiKey ?smiles ?smilesKind WHERE {
  ?compound wdt:P703 ?taxon ;
            wdt:P235 ?inchiKey .
  {
    ?compound wdt:P233 ?smiles .
    BIND("canonical" AS ?smilesKind)
  }
  UNION
  {
    ?compound wdt:P2017 ?smiles .
    BIND("isomeric" AS ?smilesKind)
  }
}
ORDER BY ?compound ?smilesKind ?smiles
"#;

/// Source statements with P703 but no P235 `InChIKey`: reported separately, never
/// deduplicated. Paged with LIMIT/OFFSET by the caller.
pub const ARCHIVE_SMILES_WITHOUT_INCHIKEY_QUERY: &str = r#"
PREFIX wdt: <http://www.wikidata.org/prop/direct/>

SELECT DISTINCT ?compound ?smiles ?smilesKind WHERE {
  ?compound wdt:P703 ?taxon .
  FILTER NOT EXISTS { ?compound wdt:P235 ?inchiKey }
  {
    ?compound wdt:P233 ?smiles .
    BIND("canonical" AS ?smilesKind)
  }
  UNION
  {
    ?compound wdt:P2017 ?smiles .
    BIND("isomeric" AS ?smilesKind)
  }
}
ORDER BY ?compound ?smilesKind ?smiles
"#;
