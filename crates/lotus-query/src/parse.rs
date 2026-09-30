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
    CompoundEntry, DatasetStats, TaxonMatch, non_empty, normalize_doi, normalize_qid,
};
use std::collections::HashSet;
use std::io::Read;
use std::sync::Arc;

/// Where each result column sits, resolved from the header row.
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
    statement: Option<usize>,
}

impl Columns {
    fn detect(headers: &csv::ByteRecord) -> Self {
        let find = |name: &str| headers.iter().position(|h| h == name.as_bytes());
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
            ref_date: find("ref_date"),
            statement: find("statement"),
        }
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
            matches.push(TaxonMatch { qid, name });
        }
    }
    Ok(matches)
}

fn field(record: &csv::ByteRecord, at: Option<usize>) -> &str {
    at.and_then(|i| record.get(i))
        .and_then(|b| std::str::from_utf8(b).ok())
        .map_or("", str::trim)
}

fn normalize_statement(value: &str) -> Option<&str> {
    non_empty(value).map(|v| {
        v.strip_prefix(lotus_model::WIKIDATA_STATEMENT_BASE)
            .unwrap_or(v)
    })
}

/// Read rows from a stream, for a result too large to hold in memory.
///
/// # Errors
/// Returns [`ParseError`] if the stream cannot be read, or if what it
/// holds cannot be read as CSV.
pub fn parse_compounds_stream<R: Read>(
    mut reader: R,
    max_rows: usize,
) -> Result<(Vec<CompoundEntry>, DatasetStats, bool), ParseError> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).map_err(ParseError::new)?;
    parse_compounds_csv_capped(&buf, max_rows)
}

#[cfg(test)]
#[path = "parse/tests.rs"]
mod tests;
