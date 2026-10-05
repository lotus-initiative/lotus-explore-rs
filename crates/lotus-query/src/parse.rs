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
    CompoundEntry, DatasetStats, TaxonMatch, TaxonNameSource, non_empty, normalize_doi,
    normalize_qid,
};
use std::collections::HashSet;
use std::sync::Arc;

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
    fn detect(headers: &csv::ByteRecord) -> Self {
        Self::resolve(|name| headers.iter().position(|h| h == name.as_bytes()))
    }

    /// Resolve every column by name, through one list of names.
    ///
    /// This was two functions -- one over borrowed byte headers for the
    /// non-streaming reader, one over `String`s for the streaming one -- and the
    /// fourteen names were written out in both. Two copies of a mapping that has
    /// to agree exactly is the shape of bug where the two paths diverge: the
    /// non-streaming path reads a column the streaming one does not, which passes
    /// every test that exercises only one of them, and fails on whichever path
    /// production happens to use.
    ///
    /// The name list now exists once. `resolve` takes the caller's own equality,
    /// because the two header types cannot be iterated as one, and nothing else
    /// about the lookup is shared.
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
    /// [`Columns::detect`] does over a borrowed header.
    #[must_use]
    fn from_names(names: &[String]) -> Self {
        Self::resolve(|name| names.iter().position(|h| h == name))
    }
}

/// Intern a value into one of the [`Interners`] sets, so that a taxon name
/// repeated across a million rows is one allocation. `$set` is the set's field
/// name, so a call site reads `interner!(self, taxon_names, value)`.
macro_rules! interner {
    ($interners:ident, $set:ident, $value:expr) => {{
        let value: &str = $value.trim();
        if value.is_empty() {
            Arc::from("")
        } else if let Some(existing) = $interners.$set.get(value) {
            Arc::clone(existing)
        } else {
            let shared: Arc<str> = Arc::from(value);
            $interners.$set.insert(Arc::clone(&shared));
            shared
        }
    }};
}

/// Interns each column's values so that a taxon name repeated across a million
/// rows is one allocation. One set per column, because a single shared set would
/// be dominated by whichever column is most repetitive.
#[derive(Default)]
struct Interners {
    qids: HashSet<Arc<str>>,
    labels: HashSet<Arc<str>>,
    taxon_names: HashSet<Arc<str>>,
    titles: HashSet<Arc<str>>,
    dois: HashSet<Arc<str>>,
    inchikeys: HashSet<Arc<str>>,
    smiles: HashSet<Arc<str>>,
    formulas: HashSet<Arc<str>>,
    statements: HashSet<Arc<str>>,
}

impl Interners {
    fn qid(interners: &mut Self, value: &str) -> Arc<str> {
        interner!(interners, qids, value)
    }

    fn label(interners: &mut Self, value: &str) -> Arc<str> {
        interner!(interners, labels, value)
    }

    fn taxon_name(interners: &mut Self, value: &str) -> Arc<str> {
        interner!(interners, taxon_names, value)
    }

    fn title(interners: &mut Self, value: &str) -> Arc<str> {
        interner!(interners, titles, value)
    }

    fn doi(interners: &mut Self, value: &str) -> Arc<str> {
        interner!(interners, dois, value)
    }

    fn inchikey(interners: &mut Self, value: &str) -> Arc<str> {
        interner!(interners, inchikeys, value)
    }

    fn smiles(interners: &mut Self, value: &str) -> Arc<str> {
        interner!(interners, smiles, value)
    }

    fn formula(interners: &mut Self, value: &str) -> Arc<str> {
        interner!(interners, formulas, value)
    }

    fn statement(interners: &mut Self, value: &str) -> Arc<str> {
        interner!(interners, statements, value)
    }
}

/// Parse rows, deduplicating on the compound-taxon-reference triple.
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
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);

    let headers = reader.byte_headers().map_err(ParseError::new)?.clone();
    let columns = Columns::detect(&headers);

    let capacity = max_rows.min(2048);
    let mut entries = Vec::with_capacity(capacity);
    let mut seen = HashSet::with_capacity(capacity.saturating_mul(2));
    let mut compounds = HashSet::with_capacity(capacity);
    let mut taxa = HashSet::with_capacity(capacity);
    let mut references = HashSet::with_capacity(capacity);
    let mut interners = Interners::default();
    let mut raw_rows = 0usize;
    let mut distinct_rows = 0usize;

    let mut record = csv::ByteRecord::new();
    while reader
        .read_byte_record(&mut record)
        .map_err(ParseError::new)?
    {
        // Owned, because `record` is reused by the next read.
        let compound = field(&record, columns.compound).to_string();
        if compound.is_empty() {
            continue;
        }
        raw_rows += 1;
        let taxon = field(&record, columns.taxon).to_string();
        let reference = field(&record, columns.reference).to_string();

        if !seen.insert((compound.clone(), taxon.clone(), reference.clone())) {
            continue;
        }
        distinct_rows += 1;
        compounds.insert(compound.clone());
        if !taxon.is_empty() {
            taxa.insert(taxon.clone());
        }
        if !reference.is_empty() {
            references.insert(reference.clone());
        }

        if entries.len() < max_rows {
            entries.push(build_entry(
                &mut interners,
                &columns,
                &record,
                &compound,
                &taxon,
                &reference,
            ));
        }
    }

    let stats = DatasetStats {
        n_compounds: compounds.len(),
        n_taxa: taxa.len(),
        n_references: references.len(),
        n_entries: raw_rows,
        n_entries_unique: distinct_rows,
    };
    let truncated = distinct_rows > entries.len();
    Ok((entries, stats, truncated))
}

fn build_entry(
    interners: &mut Interners,
    columns: &Columns,
    record: &csv::ByteRecord,
    compound: &str,
    taxon: &str,
    reference: &str,
) -> CompoundEntry {
    let iso = field(record, columns.smiles_iso);
    let conn = field(record, columns.smiles_conn);
    let smiles = if iso.is_empty() { conn } else { iso };

    // Every QID goes through `normalize_qid`. The query projects QIDs as
    // `xsd:integer(STRAFTER(STR(?x), "Q"))`, so the CSV arrives holding `16521`
    // where the item is `Q16521` -- and the row is rendered into a
    // `wikidata.org/entity/` URL. Interning the cell verbatim put the bare
    // number in the link, which is a 404 for every result row.
    CompoundEntry {
        compound_qid: Interners::qid(interners, &normalize_qid(compound)),
        name: Interners::label(interners, field(record, columns.label)),
        inchikey: optional(Interners::inchikey(
            interners,
            field(record, columns.inchikey),
        )),
        smiles: optional(Interners::smiles(interners, smiles)),
        mass: field(record, columns.mass).parse().ok(),
        formula: optional(Interners::formula(
            interners,
            field(record, columns.formula),
        )),
        taxon_qid: Interners::qid(interners, &normalize_qid(taxon)),
        taxon_name: Interners::taxon_name(interners, field(record, columns.taxon_name)),
        reference_qid: Interners::qid(interners, &normalize_qid(reference)),
        reference_node: Interners::qid(
            interners,
            normalize_reference_node(field(record, columns.reference_node)).unwrap_or_default(),
        ),
        ref_title: optional(Interners::title(
            interners,
            field(record, columns.ref_title),
        )),
        ref_doi: normalize_doi(field(record, columns.ref_doi))
            .map(|d| Interners::doi(interners, &d)),
        pub_year: field(record, columns.ref_date)
            .split(['-', 'T'])
            .next()
            .and_then(|y| y.parse().ok()),
        statement: normalize_statement(field(record, columns.statement))
            .map(|s| Interners::statement(interners, s)),
    }
}

fn optional(value: Arc<str>) -> Option<Arc<str>> {
    if value.is_empty() { None } else { Some(value) }
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

fn field(record: &csv::ByteRecord, at: Option<usize>) -> &str {
    at.and_then(|i| record.get(i))
        .and_then(|b| std::str::from_utf8(b).ok())
        .map_or("", str::trim)
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

fn normalize_statement(value: &str) -> Option<&str> {
    non_empty(value).map(|v| {
        v.strip_prefix(lotus_model::WIKIDATA_STATEMENT_BASE)
            .unwrap_or(v)
    })
}

#[path = "parse/stream.rs"]
mod stream;

pub use stream::{CsvColumnarReader, CsvSplitter, parse_compounds_columnar};

#[cfg(test)]
#[path = "parse/tests.rs"]
mod tests;
