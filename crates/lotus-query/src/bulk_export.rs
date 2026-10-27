// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
/// Pure SPARQL queries for bulk export: the LOTUS exporter, the `PubChem` tree
/// generator, and the Wikidata archive.
///
/// Every builder here returns a query string and nothing else: no HTTP, no
/// runtime, no clock. The transport is injected by the caller (see
/// [`lotus_explore_core::export::pipeline`]), so the same queries are tested
/// against a mock and run against `QLever` in production.
///
/// The queries are the ones from `lotus_exporter.py`, `lotus_pubchem_tree.py`
/// and `lotus-wikidata-archive`, transcribed to Rust. Where two sources ask the
/// same question (the compound-taxon-reference triplet, the taxon name, the
/// reference DOI) the query is written once and shared.
use lotus_model::looks_like_inchikey;

/// `QLever`'s public Wikidata SPARQL endpoint, the default for every bulk export.
pub const QLEVER_WIKIDATA: &str = "https://qlever.dev/api/wikidata";

/// Wikidata's entity IRI prefix, stripped to recover a bare QID.
pub const WIKIDATA_ENTITY_PREFIX: &str = "http://www.wikidata.org/entity/";

/// Extract a bare QID (`Q153`) from a Wikidata entity IRI.
///
/// Returns the input unchanged when it is not an entity IRI, matching the
/// Python `extract_qid` helper that all three sources replicate.
#[must_use]
pub fn extract_qid(value: &str) -> &str {
    value.strip_prefix(WIKIDATA_ENTITY_PREFIX).unwrap_or(value)
}

/// Base prefixes shared by every Wikidata query.
const PREFIXES: &str = "\
PREFIX wdt: <http://www.wikidata.org/prop/direct/>
PREFIX p: <http://www.wikidata.org/prop/>
PREFIX ps: <http://www.wikidata.org/prop/statement/>
PREFIX prov: <http://www.w3.org/ns/prov#>
PREFIX pr: <http://www.wikidata.org/prop/reference/>
PREFIX wd: <http://www.wikidata.org/entity/>
";

// ============================================================================
// LOTUS EXPORTER QUERIES (lotus_exporter.py)
// ============================================================================

/// The core triplet: compound-taxon-reference statements that carry `InChIKeys`.
///
/// Every compound that has an occurrence statement and an `InChIKey`, with the
/// taxon, reference, and reference metadata (title, DOI, date).
#[must_use]
pub fn query_compound_taxon_reference() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?compound ?compound_inchikey ?taxon ?reference WHERE {{
  ?compound wdt:P235 ?compound_inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
}}"
    )
}

/// Canonical SMILES (P233) for compounds that have an occurrence and `InChIKey`.
#[must_use]
pub fn query_compound_smiles_can() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?compound ?compound_smiles_can WHERE {{
  ?compound wdt:P235 ?inchikey ;
           wdt:P703 ?taxon ;
           wdt:P233 ?compound_smiles_can .
}}"
    )
}

/// Isomeric SMILES (P2017) for compounds that have an occurrence and `InChIKey`.
#[must_use]
pub fn query_compound_smiles_iso() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?compound ?compound_smiles_iso WHERE {{
  ?compound wdt:P235 ?inchikey ;
           wdt:P703 ?taxon ;
           wdt:P2017 ?compound_smiles_iso .
}}"
    )
}

/// `PubChem` CID (P662) for compounds that have an occurrence and `InChIKey`.
#[must_use]
pub fn query_compound_cid() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?compound ?compound_cid WHERE {{
  ?compound wdt:P235 ?inchikey ;
           wdt:P703 ?taxon ;
           wdt:P662 ?compound_cid .
}}"
    )
}

/// Taxon scientific name (P225).
#[must_use]
pub fn query_taxon_name() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?taxon ?taxon_name WHERE {{
  ?taxon wdt:P225 ?taxon_name .
}}"
    )
}

/// Taxon NCBI taxonomy ID (P685).
#[must_use]
pub fn query_taxon_ncbi() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?taxon ?taxon_ncbi WHERE {{
  ?taxon wdt:P685 ?taxon_ncbi .
}}"
    )
}

/// Taxon OTT ID (P9157).
#[must_use]
pub fn query_taxon_ott() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?taxon ?taxon_ott WHERE {{
  ?taxon wdt:P9157 ?taxon_ott .
}}"
    )
}

/// Taxon GBIF ID (P846).
#[must_use]
pub fn query_taxon_gbif() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?taxon ?taxon_gbif WHERE {{
  ?taxon wdt:P846 ?taxon_gbif .
}}"
    )
}

/// Taxon parent (P171), constrained to taxa under Biota (Q2382443).
#[must_use]
pub fn query_taxon_parent() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?taxon ?taxon_parent WHERE {{
  ?taxon wdt:P171 ?taxon_parent .
  ?taxon_parent wdt:P171* wd:Q2382443 .
}}"
    )
}

/// Taxon rank (P105) with its English label.
#[must_use]
pub fn query_taxon_rank() -> String {
    format!(
        "{PREFIXES}
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
SELECT DISTINCT ?taxon ?taxon_rank ?taxon_rank_label WHERE {{
  ?taxon wdt:P105 ?taxon_rank .
  ?taxon_rank rdfs:label ?taxon_rank_label .
  FILTER(LANG(?taxon_rank_label) = \"en\")
}}"
    )
}

/// Reference DOI (P356) for references cited by occurrence statements.
#[must_use]
pub fn query_reference_doi() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?reference ?doi WHERE {{
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P356 ?doi .
}}"
    )
}

/// Reference PMID (P698) for references cited by occurrence statements.
#[must_use]
pub fn query_reference_pmid() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?reference ?pmid WHERE {{
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P698 ?pmid .
}}"
    )
}

/// Reference PMCID (P932) for references cited by occurrence statements.
#[must_use]
pub fn query_reference_pmcid() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?reference ?pmcid WHERE {{
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P932 ?pmcid .
}}"
    )
}

/// Reference title (P1476) for references cited by occurrence statements.
#[must_use]
pub fn query_reference_title() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?reference ?title WHERE {{
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P1476 ?title .
}}"
    )
}

/// Reference publication date (P577) for references cited by occurrence statements.
#[must_use]
pub fn query_reference_date() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?reference ?date WHERE {{
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P577 ?date .
}}"
    )
}

/// Reference journal (P1433) with its label for references cited by occurrence statements.
#[must_use]
pub fn query_reference_journal() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?reference ?journal ?journal_label WHERE {{
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P1433 ?journal .
  ?journal wdt:P1476 ?journal_label .
}}"
    )
}

// ============================================================================
// PUBChem TREE QUERIES (lotus_pubchem_tree.py)
// ============================================================================

/// Compound-InChIKey-taxon triplets, the leaf compounds of the biological tree.
#[must_use]
pub fn query_compound_inchikey_taxon() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?compound ?compound_inchikey ?taxon WHERE {{
  ?compound wdt:P235 ?compound_inchikey ;
        wdt:P703 ?taxon .
}}"
    )
}

/// Compound-parent (P279) pairs, constrained to compounds under chemical compound (Q11173).
#[must_use]
pub fn query_compound_parent() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?compound ?compound_parent WHERE {{
  ?compound wdt:P279 ?compound_parent .
  ?compound_parent wdt:P279* wd:Q11173 .
}}"
    )
}

/// Compound canonical SMILES (P233).
#[must_use]
pub fn query_compound_smiles_can_all() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?compound ?compound_smiles_can WHERE {{
  ?compound wdt:P233 ?compound_smiles_can .
}}"
    )
}

/// Compound isomeric SMILES (P2017).
#[must_use]
pub fn query_compound_smiles_iso_all() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?compound ?compound_smiles_iso WHERE {{
  ?compound wdt:P2017 ?compound_smiles_iso .
}}"
    )
}

/// Compound SMARTS (P8533).
#[must_use]
pub fn query_compound_smarts() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?compound ?compound_smarts WHERE {{
  ?compound wdt:P8533 ?compound_smarts .
}}"
    )
}

/// Compound CXSMILES (P10718).
#[must_use]
pub fn query_compound_cxsmiles() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?compound ?compound_cxsmiles WHERE {{
  ?compound wdt:P10718 ?compound_cxsmiles .
}}"
    )
}

/// Compound labels (rdfs:label), English or mul.
#[must_use]
pub fn query_compound_label() -> String {
    format!(
        "PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
{PREFIXES}
SELECT DISTINCT ?compound ?compound_label ?lang WHERE {{
  ?compound wdt:P279* wd:Q11173 .
  ?compound rdfs:label ?compound_label .
  BIND(LANG(?compound_label) AS ?lang)
  FILTER (?lang IN (\"en\", \"mul\"))
}}"
    )
}

/// Reference DOI with compound and taxon, for the biological tree's reference edges.
#[must_use]
pub fn query_reference_doi_compound_taxon() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?compound ?taxon ?reference ?doi WHERE {{
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P356 ?doi .
}}"
    )
}

/// Reference PMID with compound and taxon, for the biological tree's reference edges.
#[must_use]
pub fn query_reference_pmid_compound_taxon() -> String {
    format!(
        "{PREFIXES}
SELECT DISTINCT ?compound ?taxon ?reference ?pmid WHERE {{
  ?compound wdt:P235 ?inchikey .
  ?compound p:P703 ?statement .
  ?statement ps:P703 ?taxon .
  ?statement prov:wasDerivedFrom/pr:P248 ?reference .
  ?reference wdt:P698 ?pmid .
}}"
    )
}

// ============================================================================
// WIKIDATA ARCHIVE QUERIES (lotus-wikidata-archive)
// ============================================================================

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

/// Page size for stable `QLever` pagination of the archive queries.
pub const ARCHIVE_PAGE_SIZE: usize = 50_000;

/// Append `LIMIT`/`OFFSET` to a base query for one stable page.
#[must_use]
pub fn archive_page(base_query: &str, offset: usize) -> String {
    format!("{base_query}\nLIMIT {ARCHIVE_PAGE_SIZE}\nOFFSET {offset}")
}

/// Whether a value looks like a standard `InChIKey`, used to validate decoded rows.
#[must_use]
pub fn looks_like_archive_inchikey(value: &str) -> bool {
    looks_like_inchikey(value)
}

#[cfg(test)]
#[path = "bulk_export/tests.rs"]
mod tests;
