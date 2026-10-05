// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Reading `QLever`'s CSV into domain types.
//!
//! The parsers are deliberately forgiving. `flexible(true)` plus per-field
//! defaulting means a truncated or oddly-shaped payload degrades to fewer or
//! emptier columns rather than failing: a search that has already returned
//! usable rows should not be thrown away because one cell was unparsable.

use crate::error::ParseError;
use lotus_model::{
    CompoundEntry, DatasetStats, TaxonMatch, TaxonNameSource, non_empty, normalize_qid,
};

/// Where each result column sits, resolved from the header row.
#[derive(Debug, Clone, Copy)]
struct Columns {
    compound: Option<usize>,
    label: Option<usize>,
    inchikey: Option<usize>,
    smiles_iso: Option<usize>,
    smiles_conn: Option<usize>,
    mass: Option<usize>,
    formula: Option<usize>,
    taxon: Option<usize>,
    taxon_name: Option<usize>,
    reference: Option<usize>,
    ref_title: Option<usize>,
    ref_doi: Option<usize>,
    ref_date: Option<usize>,
    /// The reference *node*, distinct from `ref_qid` (the stated-in publication).
    reference_node: Option<usize>,
    statement: Option<usize>,
}

impl Columns {
    /// Resolve every column by name, through one list of names.
    ///
    /// The name list exists once. `resolve` takes the caller's own equality,
    /// because the header types cannot be iterated as one, and nothing else about
    /// the lookup is shared.
    ///
    /// This was two functions -- one over borrowed byte headers, one over owned
    /// `String`s -- with the names written out in both. Two copies of a mapping
    /// that has to agree exactly is the shape of bug where the two paths diverge:
    /// one path reads a column the other does not, which passes every test that
    /// exercises only one of them.
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
}

impl Columns {
    /// Whether any column the result set needs was found in the header.
    ///
    /// A header naming no known column means the query and the parser disagree,
    /// and every row would read as empty. That is a failure the table cannot show
    /// as a failure, so it is reported instead.
    /// Whether the header named anything this reader can use.
    ///
    /// Every column that can be resolved belongs here, not just the identifying
    /// ones: a header naming only `ref_node` is a payload we understand, and
    /// refusing it would report a shape mismatch where there is none.
    const fn resolves_any(&self) -> bool {
        self.compound.is_some()
            || self.taxon.is_some()
            || self.reference.is_some()
            || self.statement.is_some()
            || self.reference_node.is_some()
    }

    /// Resolve the columns from header names already read as strings.
    ///
    /// The streaming reader sees its header as owned `String`s because it has
    /// to hand the names to a caller; this is the same resolution
    #[must_use]
    fn from_names(names: &[String]) -> Self {
        Self::resolve(|name| names.iter().position(|h| h == name))
    }
}

/// Parse rows into entries, through the one result reader.
///
/// # Errors
/// Returns [`ParseError`] if the payload cannot be read as CSV. A payload
/// that reads but has the wrong shape does not: missing columns become empty.
pub fn parse_compounds_csv(
    bytes: &[u8],
    max_rows: usize,
) -> Result<Vec<CompoundEntry>, ParseError> {
    let (rows, _, _) = parse_compounds_csv_capped(bytes, max_rows)?;
    Ok(rows)
}

/// Parse rows and count the whole result set, whether or not the rows were kept.
///
/// This is the reader to prefer: the counts it returns are the endpoint's, and
/// `capped` says whether the display limit hid part of the result. A caller
/// that counts from the returned rows cannot reproduce the raw count, because
/// deduplication happens during parsing.
///
/// # Errors
/// Returns [`ParseError`] if the payload cannot be read as CSV.
pub fn parse_compounds_csv_capped(
    bytes: &[u8],
    max_rows: usize,
) -> Result<(Vec<CompoundEntry>, DatasetStats, bool), ParseError> {
    // Delegates to the streaming reader, which the app already uses.
    //
    // There were two implementations of "read a result set" in this crate and
    // three consumers: the CLI and the server API through here, the app in the
    // browser and natively through the streaming reader. They disagreed, and the
    // disagreement was visible rather than theoretical -- on a payload with one
    // repeated row this returned 3 rows where the browser returned 4, because
    // this deduplicated on the compound-taxon-reference triple and the set did not.
    //
    // So the surviving path is the one that does not deduplicate, and that is a
    // decision rather than an accident: the query is `SELECT DISTINCT`, so a real
    // payload has no duplicate rows for a deduplicating pass to remove, and a row
    // that pass used to drop is a row the graph actually holds. The streaming
    // reader also builds the `ColumnarResultSet` everything else reads, and it
    // streams rather than needing the whole payload in memory.
    //
    // `tests/result_parsing.rs` runs the same bytes through both and compares them
    // row for row, so this cannot drift back.
    // An empty body is an empty result, not a malformed one. The streaming reader
    // refuses a payload with no header row, which is right for a truncated
    // response and wrong for an endpoint that answered with nothing: a caller
    // showing "0 results" for an empty answer is correct, and turning that into a
    // parse error turns a quiet answer into a red one.
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok((Vec::new(), DatasetStats::default(), false));
    }

    let set = stream::parse_compounds_columnar(bytes)?;

    // `max_rows` still caps. The set holds every row the payload carried, so
    // without this the caller's bound stops bounding memory -- which matters for
    // the server API, where the payload is the endpoint's whole answer rather than
    // a page of it.
    let count = set.row_count();
    let capped = if max_rows == usize::MAX {
        count
    } else {
        max_rows
    };
    let mut rows = Vec::with_capacity(count.min(2048));
    for row in 0..count.min(capped) {
        if let Some(entry) = set.entry(row) {
            rows.push(entry);
        }
    }

    // Capping the returned rows is what `truncated` means: the payload held more
    // than the caller asked to keep. The endpoint may also have applied its own
    // limit, which this cannot see and does not guess at.
    let truncated = count > rows.len();

    Ok((rows, set.stats(), truncated))
}

/// Read the single-row `COUNT` result.
///
/// A reported `n_entries_unique` of zero means the count subquery did not
/// evaluate, not that every row is a duplicate; reporting the raw count is the
/// lesser wrong answer.
///
/// # Errors
/// Returns [`ParseError`] if the payload has no data row, because there
/// is then nothing to fall back on: a total cannot be invented.
pub fn parse_counts_csv(bytes: &[u8]) -> Result<DatasetStats, ParseError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);

    let headers = reader.headers().map_err(ParseError::new)?.clone();
    let index = |name: &str| headers.iter().position(|h| h == name);

    let mut records = reader.records();
    let record = match records.next() {
        Some(Ok(r)) => r,
        Some(Err(e)) => return Err(ParseError::new(e)),
        None => return Err(ParseError::new("the count query returned no row")),
    };
    let read = |name: &str| -> usize {
        index(name)
            .and_then(|i| record.get(i))
            .map(str::trim)
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };

    let n_entries = read("n_entries");
    let reported_unique = read("n_entries_unique");

    Ok(DatasetStats {
        n_entries,
        n_entries_unique: if reported_unique == 0 {
            n_entries
        } else {
            reported_unique
        },
        n_compounds: read("n_compounds"),
        n_taxa: read("n_taxa"),
        n_references: read("n_references"),
    })
}

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
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);

    let headers = reader.headers().map_err(ParseError::new)?.clone();
    let qid_at = headers.iter().position(|h| h == "taxon");
    let name_at = headers.iter().position(|h| h == "taxon_name");
    let source_at = headers.iter().position(|h| h == "matched_by");

    let mut matches = Vec::new();
    for record in reader.records() {
        let record = record.map_err(ParseError::new)?;
        let qid = normalize_qid(record.get(qid_at.unwrap_or(usize::MAX)).unwrap_or(""));
        let name = record
            .get(name_at.unwrap_or(usize::MAX))
            .unwrap_or("")
            .trim()
            .to_string();
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

/// Read the `?matched_by` token [`taxon_lookup_query`] binds.
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
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);
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
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);

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

fn text_field(record: &csv::StringRecord, at: Option<usize>) -> String {
    record
        .get(at.unwrap_or(usize::MAX))
        .unwrap_or("")
        .trim()
        .to_owned()
}

/// The reference node's identity, with the namespace stripped.
///
/// The remainder is the 64-hex node hash, which is what identifies the reference.
fn normalize_reference_node(value: &str) -> Option<&str> {
    non_empty(value).map(|v| {
        v.strip_prefix(lotus_model::WIKIDATA_REFERENCE_BASE)
            .unwrap_or(v)
    })
}

#[path = "parse/stream.rs"]
mod stream;

pub use stream::{CsvColumnarReader, CsvSplitter, parse_compounds_columnar};

#[cfg(test)]
#[path = "parse/tests.rs"]
mod tests;
