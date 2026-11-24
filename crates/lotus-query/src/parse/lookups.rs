// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! CSV parsers for taxon, reference, and compound resolution responses.

use lotus_model::{TaxonMatch, TaxonNameSource, normalize_qid};

use super::columns::{open_csv_reader, text_field};
use crate::error::ParseError;

/// Parse taxon lookup rows, dropping any that has no QID or no name.
///
/// `?matched_by` is read when the payload carries it and defaults to
/// [`TaxonNameSource::Scientific`] when it does not, so a payload from before
/// the common-name branch existed still parses — and still parses as the
/// optimistic reading, which is the one that needs no notice.
///
/// # Errors
/// Returns [`ParseError`] if the payload cannot be read as CSV.
pub fn parse_taxon_csv(bytes: &[u8]) -> Result<Vec<TaxonMatch>, ParseError> {
    let mut reader = open_csv_reader(bytes);

    let headers = reader.headers().map_err(ParseError::new)?.clone();
    let qid_at = headers.iter().position(|h| h == "taxon");
    let name_at = headers.iter().position(|h| h == "taxon_name");
    let source_at = headers.iter().position(|h| h == "matched_by");

    let mut matches = Vec::new();
    for record in reader.records() {
        let record = record.map_err(ParseError::new)?;
        let qid = normalize_qid(record.get(qid_at.unwrap_or(usize::MAX)).unwrap_or(""));
        let name = text_field(&record, name_at);
        if !qid.is_empty() && !name.is_empty() {
            matches.push(TaxonMatch {
                qid,
                name,
                source: taxon_name_source(
                    record.get(source_at.unwrap_or(usize::MAX)).unwrap_or(""),
                ),
            });
        }
    }
    Ok(matches)
}

/// Read the `?matched_by` token [`taxon_lookup_query`](crate::query::taxon_lookup_query) binds.
///
/// Anything that is not the one token it uses for a common name is read as a
/// scientific name, because that is the safe direction: a notice about a common
/// name is worth showing only when the endpoint actually said so.
fn taxon_name_source(token: &str) -> TaxonNameSource {
    match token.trim() {
        "common" => TaxonNameSource::Common,
        _ => TaxonNameSource::Scientific,
    }
}

/// A reference a lookup matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceMatch {
    /// Wikidata QID, taken from the item URI.
    pub qid: String,
}

/// Parse reference-lookup rows.
///
/// Rows with no item are dropped rather than kept as an empty match: a lookup
/// that returned one would mean a reference was found where there is none, and
/// the caller would search for compounds reported by nothing.
///
/// # Errors
/// Returns [`ParseError`] if the payload cannot be read as CSV.
pub fn parse_reference_lookup_csv(bytes: &[u8]) -> Result<Vec<ReferenceMatch>, ParseError> {
    let mut reader = open_csv_reader(bytes);
    let headers = reader.headers().map_err(ParseError::new)?.clone();
    let at = headers.iter().position(|h| h.trim() == "ref");

    let mut matches = Vec::new();
    for record in reader.records() {
        let record = record.map_err(ParseError::new)?;
        let qid = normalize_qid(&text_field(&record, at));
        if !qid.is_empty() {
            matches.push(ReferenceMatch { qid });
        }
    }
    Ok(matches)
}

/// One compound a structure lookup matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompoundMatch {
    /// Wikidata QID.
    pub qid: String,
    /// The English label, when the item has one, for the notice and the
    /// ambiguity list. May be empty: a compound with no English label is still
    /// a legitimate match.
    pub label: String,
    /// `P233`, the canonical SMILES, when the item has one.
    ///
    /// This is what the structure service is handed when the reader asks for a
    /// substructure or a similarity search on an input that named a compound: the
    /// compound's own structure, not whatever the reader typed. Empty means the
    /// item has no canonical SMILES, and the reader's own input is used instead.
    pub canonical_smiles: String,
}

/// Parse compound-lookup rows, dropping any that has no QID.
///
/// # Errors
/// Returns [`ParseError`] if the payload cannot be read as CSV.
pub fn parse_compound_lookup_csv(bytes: &[u8]) -> Result<Vec<CompoundMatch>, ParseError> {
    let mut reader = open_csv_reader(bytes);

    let headers = reader.headers().map_err(ParseError::new)?.clone();
    let qid_at = headers.iter().position(|h| h == "compound_qid");
    let label_at = headers.iter().position(|h| h == "compound_label");
    let smiles_at = headers.iter().position(|h| h == "canonical_smiles");

    let mut matches = Vec::new();
    for record in reader.records() {
        let record = record.map_err(ParseError::new)?;
        let qid = normalize_qid(&text_field(&record, qid_at));
        if qid.is_empty() {
            continue;
        }
        matches.push(CompoundMatch {
            qid,
            label: text_field(&record, label_at),
            canonical_smiles: text_field(&record, smiles_at),
        });
    }
    Ok(matches)
}
