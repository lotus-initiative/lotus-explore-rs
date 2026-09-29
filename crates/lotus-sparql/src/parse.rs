// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Reading `QLever`'s CSV into domain types.
//!
//! The parsers are deliberately forgiving. `flexible(true)` plus per-field
//! defaulting means a truncated or oddly-shaped payload degrades to fewer or
//! emptier columns rather than failing: a search that has already returned
//! usable rows should not be thrown away because one cell was unparseable.

use crate::error::FetchError;
use lotus_core::{
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
/// Returns [`FetchError::Parse`] if the payload cannot be read as CSV. A payload
/// that reads but has the wrong shape does not: missing columns become empty.
pub fn parse_compounds_csv(
    bytes: &[u8],
    max_rows: usize,
) -> Result<Vec<CompoundEntry>, FetchError> {
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
/// Returns [`FetchError::Parse`] if the payload cannot be read as CSV.
pub fn parse_compounds_csv_capped(
    bytes: &[u8],
    max_rows: usize,
) -> Result<(Vec<CompoundEntry>, DatasetStats, bool), FetchError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);

    let headers = reader
        .byte_headers()
        .map_err(|e| FetchError::Parse(e.to_string()))?
        .clone();
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
        .map_err(|e| FetchError::Parse(e.to_string()))?
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

    CompoundEntry {
        compound_qid: Interners::qid(interners, compound),
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
        taxon_qid: Interners::qid(interners, taxon),
        taxon_name: Interners::taxon_name(interners, field(record, columns.taxon_name)),
        reference_qid: Interners::qid(interners, reference),
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
/// Returns [`FetchError::Parse`] if the payload has no data row, because there
/// is then nothing to fall back on: a total cannot be invented.
pub fn parse_counts_csv(bytes: &[u8]) -> Result<DatasetStats, FetchError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);

    let headers = reader
        .headers()
        .map_err(|e| FetchError::Parse(e.to_string()))?
        .clone();
    let index = |name: &str| headers.iter().position(|h| h == name);

    let mut records = reader.records();
    let record = match records.next() {
        Some(Ok(r)) => r,
        Some(Err(e)) => return Err(FetchError::Parse(e.to_string())),
        None => return Err(FetchError::Parse("the count query returned no row".into())),
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
/// Returns [`FetchError::Parse`] if the payload cannot be read as CSV.
pub fn parse_taxon_csv(bytes: &[u8]) -> Result<Vec<TaxonMatch>, FetchError> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);

    let headers = reader
        .headers()
        .map_err(|e| FetchError::Parse(e.to_string()))?
        .clone();
    let qid_at = headers.iter().position(|h| h == "taxon");
    let name_at = headers.iter().position(|h| h == "taxon_name");

    let mut matches = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|e| FetchError::Parse(e.to_string()))?;
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
        v.strip_prefix(lotus_core::WIKIDATA_STATEMENT_BASE)
            .unwrap_or(v)
    })
}

/// Read rows from a stream, for a result too large to hold in memory.
///
/// # Errors
/// Returns [`FetchError::Parse`] if the stream cannot be read, or if what it
/// holds cannot be read as CSV.
pub fn parse_compounds_stream<R: Read>(
    mut reader: R,
    max_rows: usize,
) -> Result<(Vec<CompoundEntry>, DatasetStats, bool), FetchError> {
    let mut buf = Vec::new();
    reader
        .read_to_end(&mut buf)
        .map_err(|e| FetchError::Parse(e.to_string()))?;
    parse_compounds_csv_capped(&buf, max_rows)
}

#[cfg(test)]
mod tests {
    // A test asserting on a fixture may panic when the fixture is wrong: that
    // is the failure it is reporting, and the lint exists to keep library code
    // free of panics on external input.
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
    #![allow(clippy::format_collect)]

    use super::*;
    use lotus_core::WIKIDATA_STATEMENT_BASE;

    const HEADER: &str = "compound,compoundLabel,compound_inchikey,compound_smiles_conn,compound_smiles_iso,compound_mass,compound_formula,taxon,taxon_name,ref_qid,ref_title,ref_doi,ref_date,statement";

    fn csv(rows: &str) -> Vec<u8> {
        format!("{HEADER}\n{rows}").into_bytes()
    }

    #[test]
    fn the_iso_smiles_wins_over_the_connection_table_one() {
        let rows = parse_compounds_csv(
            &csv("Q1,L,IK,CC=CC,CC=CC,78.0,C6H6,Q10,T,Q100,Title,10.1/a,2021,http://x/S1\n"),
            10,
        )
        .expect("valid CSV");
        assert_eq!(rows[0].smiles.as_deref(), Some("CC=CC"));
    }

    #[test]
    fn the_connection_table_smiles_is_used_when_there_is_no_iso_form() {
        let rows = parse_compounds_csv(
            &csv("Q1,L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,Title,10.1/a,2021,\n"),
            10,
        )
        .expect("valid CSV");
        assert_eq!(rows[0].smiles.as_deref(), Some("CC=CC"));
    }

    #[test]
    fn a_doi_prefix_is_stripped_but_a_reference_title_is_not() {
        let rows = parse_compounds_csv(
            &csv("Q1,L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,https://doi.org/10.1/B,10.1/B,2021,\n"),
            10,
        )
        .expect("valid CSV");
        assert_eq!(rows[0].ref_title.as_deref(), Some("https://doi.org/10.1/B"));
        assert_eq!(rows[0].ref_doi.as_deref(), Some("10.1/B"));
    }

    #[test]
    fn a_publication_date_contributes_only_its_year() {
        for (input, expected) in [("2021-04-23T00:00:00Z", Some(2021)), ("2019", Some(2019))] {
            let rows = parse_compounds_csv(
                &csv(&format!(
                    "Q1,L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,T,10.1/a,{input},\n"
                )),
                10,
            )
            .expect("valid CSV");
            assert_eq!(rows[0].pub_year, expected, "input {input}");
        }
    }

    #[test]
    fn a_row_without_a_compound_id_is_not_a_result() {
        let rows = parse_compounds_csv(
            &csv(",L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,T,10.1/a,2021,\n"),
            10,
        )
        .expect("valid CSV");
        assert!(rows.is_empty());
    }

    #[test]
    fn a_row_missing_several_columns_still_parses() {
        let rows = parse_compounds_csv(&csv("Q1,L\n"), 10).expect("valid CSV");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].taxon_qid.is_empty());
        assert!(rows[0].smiles.is_none());
    }

    #[test]
    fn a_statement_uri_loses_its_prefix() {
        let uri = format!("{WIKIDATA_STATEMENT_BASE}S1");
        let rows = parse_compounds_csv(
            &csv(&format!(
                "Q1,L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,T,10.1/a,2021,{uri}\n"
            )),
            10,
        )
        .expect("valid CSV");
        assert_eq!(rows[0].statement.as_deref(), Some("S1"));
    }

    #[test]
    fn a_duplicate_triple_is_kept_once_and_counted_once_each_way() {
        let row = "Q1,L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,T,10.1/a,2021,\n";
        let (rows, stats, capped) =
            parse_compounds_csv_capped(&csv(&format!("{row}{row}")), 10).expect("valid CSV");
        assert_eq!(rows.len(), 1);
        assert_eq!(stats.n_entries, 2, "raw rows");
        assert_eq!(stats.n_entries_unique, 1, "distinct triples");
        assert!(!capped);
    }

    #[test]
    fn capping_hides_rows_but_not_counts() {
        // Five distinct compounds, so the cap hides rows rather than duplicates.
        let rows: String = (1..=5)
            .map(|i| format!("Q{i},L,IK,CC=CC,,78.0,C6H6,Q10,T,Q100,T,10.1/a,2021,\n"))
            .collect();
        let (kept, stats, capped) = parse_compounds_csv_capped(&csv(&rows), 2).expect("valid CSV");
        assert_eq!(kept.len(), 2);
        assert!(
            capped,
            "the caller must be able to say the table is partial"
        );
        assert_eq!(stats.n_entries, 5);
        assert_eq!(stats.n_compounds, 5);
    }

    #[test]
    fn a_zero_unique_count_falls_back_to_the_entry_count() {
        let bytes = b"n_entries,n_entries_unique,n_compounds,n_taxa,n_references\n10,0,3,2,4\n";
        let stats = parse_counts_csv(bytes).expect("valid CSV");
        assert_eq!(stats.n_entries_unique, 10);
    }

    #[test]
    fn a_count_query_with_no_row_is_an_error() {
        // Unlike a row payload, there is nothing to fall back on: a total
        // cannot be invented.
        let err = parse_counts_csv(b"n_entries\n").expect_err("header only");
        assert!(matches!(err, FetchError::Parse(_)));
    }

    #[test]
    fn taxon_rows_need_both_an_id_and_a_name() {
        let bytes = b"taxon,taxon_name\nhttp://www.wikidata.org/entity/Q1,A\nQ2,\n,Q3\n";
        let matches = parse_taxon_csv(bytes).expect("valid CSV");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].qid, "Q1");
    }

    #[test]
    fn an_empty_payload_is_an_empty_result_not_an_error() {
        assert!(
            parse_compounds_csv(b"", 10)
                .expect("empty is valid")
                .is_empty()
        );
    }
}
