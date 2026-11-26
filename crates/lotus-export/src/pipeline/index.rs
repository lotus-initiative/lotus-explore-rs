// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Per-QID indexing of the four LOTUS export tables.

use std::collections::HashMap;

use super::ExportError;

/// One non-deprecated occurrence triplet from
/// [`COMPOUND_TAXON_REFERENCE_QUERY`](lotus_query::COMPOUND_TAXON_REFERENCE_QUERY),
/// with QIDs already stripped of their entity IRI prefixes.
pub(super) struct TripletRow {
    pub(super) compound_qid: String,
    pub(super) compound_inchikey: String,
    pub(super) taxon_qid: String,
    pub(super) reference_qid: String,
}

/// All of one compound's structure properties, keyed by compound QID.
#[derive(Clone, Default)]
pub(super) struct CompoundProps {
    pub(super) smiles_can: Option<String>,
    pub(super) smiles_iso: Option<String>,
    pub(super) cid: Option<String>,
}

/// All of one taxon's properties, keyed by taxon QID.
#[derive(Clone, Default)]
pub(super) struct TaxonProps {
    pub(super) name: Option<String>,
    pub(super) ncbi: Option<String>,
    pub(super) ott: Option<String>,
    pub(super) gbif: Option<String>,
    pub(super) parent: Option<String>,
    pub(super) rank: Option<String>,
    pub(super) rank_label: Option<String>,
}

/// All of one reference's properties, keyed by reference QID.
#[derive(Clone, Default)]
pub(super) struct ReferenceProps {
    pub(super) doi: Option<String>,
    pub(super) pmid: Option<String>,
    pub(super) pmcid: Option<String>,
    pub(super) title: Option<String>,
    pub(super) date: Option<String>,
    pub(super) journal: Option<String>,
    pub(super) journal_label: Option<String>,
}

/// Read a named column from `record`, returning `""` when the column is absent.
fn record_col<'a>(
    headers: &csv::StringRecord,
    record: &'a csv::StringRecord,
    name: &str,
) -> &'a str {
    headers
        .iter()
        .position(|h| h == name)
        .and_then(|i| record.get(i))
        .unwrap_or("")
}

/// Overwrite `slot` when `record` carries a non-empty value for `name`.
fn set_col(
    slot: &mut Option<String>,
    headers: &csv::StringRecord,
    record: &csv::StringRecord,
    name: &str,
) {
    let val = record_col(headers, record, name);
    if !val.is_empty() {
        *slot = Some(val.to_string());
    }
}

fn require_col(
    headers: &csv::StringRecord,
    table: &'static str,
    column: &'static str,
) -> Result<(), ExportError> {
    if headers.iter().any(|h| h == column) {
        Ok(())
    } else {
        Err(ExportError::MissingColumn { table, column })
    }
}

/// Index a CSV payload by QID without allocating per-row `HashMap`s.
fn index_csv_records<V: Default>(
    bytes: &[u8],
    table: &'static str,
    key_col: &'static str,
    mut populate: impl FnMut(&mut V, &csv::StringRecord, &csv::StringRecord),
) -> Result<HashMap<String, V>, ExportError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    require_col(&headers, table, key_col)?;
    let mut index = HashMap::new();
    for record in reader.records() {
        let record = record?;
        let key = lotus_query::extract_qid(record_col(&headers, &record, key_col)).to_string();
        if key.is_empty() {
            continue;
        }
        populate(index.entry(key).or_default(), &headers, &record);
    }
    Ok(index)
}

/// Parse the occurrence triplets table directly into [`TripletRow`] records.
pub(super) fn read_triplets(bytes: &[u8]) -> Result<Vec<TripletRow>, ExportError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);
    let headers = reader.headers()?.clone();
    for column in ["compound", "compound_inchikey", "taxon", "reference"] {
        require_col(&headers, "compound_taxon_reference", column)?;
    }
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        rows.push(TripletRow {
            compound_qid: lotus_query::extract_qid(record_col(&headers, &record, "compound"))
                .to_string(),
            compound_inchikey: record_col(&headers, &record, "compound_inchikey").to_string(),
            taxon_qid: lotus_query::extract_qid(record_col(&headers, &record, "taxon")).to_string(),
            reference_qid: lotus_query::extract_qid(record_col(&headers, &record, "reference"))
                .to_string(),
        });
    }
    Ok(rows)
}

/// Build `compound QID → CompoundProps` from the consolidated compounds CSV.
pub(super) fn build_compound_index(
    bytes: &[u8],
) -> Result<HashMap<String, CompoundProps>, ExportError> {
    index_csv_records(
        bytes,
        "compounds",
        "compound",
        |entry: &mut CompoundProps, headers, record| {
            set_col(
                &mut entry.smiles_can,
                headers,
                record,
                "compound_smiles_can",
            );
            set_col(
                &mut entry.smiles_iso,
                headers,
                record,
                "compound_smiles_iso",
            );
            set_col(&mut entry.cid, headers, record, "compound_cid");
        },
    )
}

/// Build `taxon QID → TaxonProps` from the consolidated taxa CSV.
pub(super) fn build_taxon_index(bytes: &[u8]) -> Result<HashMap<String, TaxonProps>, ExportError> {
    index_csv_records(
        bytes,
        "taxa",
        "taxon",
        |entry: &mut TaxonProps, headers, record| {
            set_col(&mut entry.name, headers, record, "taxon_name");
            set_col(&mut entry.ncbi, headers, record, "taxon_ncbi");
            set_col(&mut entry.ott, headers, record, "taxon_ott");
            set_col(&mut entry.gbif, headers, record, "taxon_gbif");
            set_col(&mut entry.parent, headers, record, "taxon_parent");
            set_col(&mut entry.rank, headers, record, "taxon_rank");
            set_col(&mut entry.rank_label, headers, record, "taxon_rank_label");
        },
    )
}

/// Build `reference QID → ReferenceProps` from the consolidated references CSV.
pub(super) fn build_reference_index(
    bytes: &[u8],
) -> Result<HashMap<String, ReferenceProps>, ExportError> {
    index_csv_records(
        bytes,
        "references",
        "reference",
        |entry: &mut ReferenceProps, headers, record| {
            set_col(&mut entry.doi, headers, record, "doi");
            set_col(&mut entry.pmid, headers, record, "pmid");
            set_col(&mut entry.pmcid, headers, record, "pmcid");
            set_col(&mut entry.title, headers, record, "title");
            set_col(&mut entry.date, headers, record, "date");
            set_col(&mut entry.journal, headers, record, "journal");
            set_col(&mut entry.journal_label, headers, record, "journal_label");
        },
    )
}
