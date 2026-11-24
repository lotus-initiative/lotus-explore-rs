// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Pure SPARQL queries for bulk export: the LOTUS exporter, the `PubChem` tree
//! generator, and the Wikidata archive.
//!
//! Every query here is a `const` string and nothing else: no HTTP, no
//! runtime, no clock. The transport is injected by the caller (see
//! `lotus_export::pipeline`), so the same queries are tested
//! against a mock and run against `QLever` in production.
//!
//! The queries are the ones from `lotus_exporter.py`, `lotus_pubchem_tree.py`
//! and `lotus-wikidata-archive`, transcribed to Rust. Where two sources ask the
//! same question (the compound-taxon-reference triplet, the taxon name, the
//! reference DOI) the query is written once and shared.

/// Base prefixes shared by every Wikidata query.
///
/// A macro rather than a `const` because `concat!` only accepts literals, and
/// `concat!` is what lets every query below be a `const`: a query that is data
/// has no function body for a mutation to replace, so its exact text is pinned
/// by one table-driven test instead of one test per builder.
macro_rules! prefixes {
    () => {
        "\
PREFIX wdt: <http://www.wikidata.org/prop/direct/>
PREFIX p: <http://www.wikidata.org/prop/>
PREFIX ps: <http://www.wikidata.org/prop/statement/>
PREFIX prov: <http://www.w3.org/ns/prov#>
PREFIX pr: <http://www.wikidata.org/prop/reference/>
PREFIX wd: <http://www.wikidata.org/entity/>
PREFIX wikibase: <http://wikiba.se/ontology#>
"
    };
}

/// `SELECT DISTINCT <vars> WHERE { <lines> }` under the base prefixes, one
/// pattern line per argument.
macro_rules! select_distinct {
    ($vars:expr, $($line:expr),+ $(,)?) => {
        concat!(
            prefixes!(),
            "\nSELECT DISTINCT ", $vars, " WHERE {\n",
            $($line, "\n",)+
            "}"
        )
    };
}

/// One direct property of every subject that has it: `?<subject> wdt:<P> ?<column>`.
macro_rules! direct_property {
    ($subject:literal, $property:literal, $column:literal) => {
        select_distinct!(
            concat!("?", $subject, " ?", $column),
            concat!("  ?", $subject, " wdt:", $property, " ?", $column, " .")
        )
    };
}

/// Properties of the references cited by occurrence statements (compound with
/// an `InChIKey`, found in a taxon, stated in a reference).
macro_rules! occurrence_reference {
    ($vars:expr, $($line:expr),+ $(,)?) => {
        select_distinct!(
            $vars,
            "  ?compound wdt:P235 ?inchikey .",
            "  ?compound p:P703 ?statement .",
            "  ?statement ps:P703 ?taxon .",
            "  ?statement wikibase:rank ?_rank .",
            "  FILTER(?_rank != wikibase:DeprecatedRank)",
            "  ?statement prov:wasDerivedFrom/pr:P248 ?reference .",
            $($line,)+
        )
    };
}

mod parse_id;
mod queries_core;
mod queries_enrich;

#[cfg(test)]
mod tests;

pub use parse_id::{ARCHIVE_PAGE_SIZE, archive_page, extract_qid};
pub use queries_core::{
    COMPOUND_TAXON_REFERENCE_QUERY, EXPORT_COMPOUNDS_QUERY, EXPORT_REFERENCES_QUERY,
    EXPORT_TAXA_QUERY, TAXON_NAME_QUERY, TAXON_NCBI_QUERY, TAXON_PARENT_QUERY,
};
pub use queries_enrich::{
    ARCHIVE_SMILES_QUERY, ARCHIVE_SMILES_WITHOUT_INCHIKEY_QUERY, COMPOUND_CXSMILES_QUERY,
    COMPOUND_INCHIKEY_TAXON_QUERY, COMPOUND_LABEL_QUERY, COMPOUND_PARENT_QUERY,
    COMPOUND_SMARTS_QUERY, COMPOUND_SMILES_CAN_ALL_QUERY, COMPOUND_SMILES_ISO_ALL_QUERY,
    REFERENCE_DOI_COMPOUND_TAXON_QUERY, REFERENCE_PMID_COMPOUND_TAXON_QUERY,
};
