// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Header column resolution and low-level CSV reader construction.

use lotus_model::non_empty;

/// Where each result column sits, resolved from the header row.
#[derive(Debug, Clone, Copy)]
pub(super) struct Columns {
    pub(super) compound: Option<usize>,
    pub(super) label: Option<usize>,
    pub(super) inchikey: Option<usize>,
    pub(super) smiles_iso: Option<usize>,
    pub(super) smiles_conn: Option<usize>,
    pub(super) mass: Option<usize>,
    pub(super) formula: Option<usize>,
    pub(super) taxon: Option<usize>,
    pub(super) taxon_name: Option<usize>,
    pub(super) reference: Option<usize>,
    pub(super) ref_title: Option<usize>,
    pub(super) ref_doi: Option<usize>,
    pub(super) ref_date: Option<usize>,
    /// The reference *node*, distinct from `ref_qid` (the stated-in publication).
    pub(super) reference_node: Option<usize>,
    pub(super) statement: Option<usize>,
}

impl Columns {
    /// Resolve every column by name, through one list of names.
    fn resolve(mut find: impl FnMut(&str) -> Option<usize>) -> Self {
        Self {
            compound: find("compound"),
            label: find("compoundLabel"),
            inchikey: find("compound_inchikey"),
            smiles_iso: find("compound_smiles_iso"),
            smiles_conn: find("compound_smiles_conn"),
            mass: find("compound_mass"),
            formula: find("compound_formula"),
            taxon: find("taxon"),
            taxon_name: find("taxon_name"),
            reference: find("ref_qid"),
            ref_title: find("ref_title"),
            ref_doi: find("ref_doi"),
            ref_date: find("ref_year"),
            reference_node: find("ref_node"),
            statement: find("statement_id"),
        }
    }

    /// Whether the header named anything this reader can use.
    pub(super) const fn resolves_any(&self) -> bool {
        self.compound.is_some()
            || self.taxon.is_some()
            || self.reference.is_some()
            || self.statement.is_some()
            || self.reference_node.is_some()
    }

    /// Resolve the columns from header names already read as strings.
    #[must_use]
    pub(super) fn from_names(names: &[String]) -> Self {
        Self::resolve(|name| names.iter().position(|h| h == name))
    }
}

pub(super) fn strip_utf8_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)
}

pub(super) fn open_csv_reader(bytes: &[u8]) -> csv::Reader<&[u8]> {
    csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(strip_utf8_bom(bytes))
}

pub(super) fn text_field(record: &csv::StringRecord, at: Option<usize>) -> String {
    record
        .get(at.unwrap_or(usize::MAX))
        .unwrap_or("")
        .trim()
        .to_owned()
}

/// The reference node's identity, with the namespace stripped.
///
/// The remainder is the 64-hex node hash, which is what identifies the reference.
pub(super) fn normalize_reference_node(value: &str) -> Option<&str> {
    non_empty(value).map(|v| {
        v.strip_prefix(lotus_model::WIKIDATA_REFERENCE_BASE)
            .unwrap_or(v)
    })
}
