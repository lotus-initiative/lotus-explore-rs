// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Writing a result set to stdout.
//!
//! Data goes to stdout, diagnostics to stderr, so that `lotus search … > out.csv`
//! captures the data and nothing else.

use std::io::Write;

use clap::ValueEnum;
use lotus_model::CompoundEntry;
use lotus_search::SearchResult;
use sha2::Digest;

/// The columns a table or a delimited file carries, in order.
const COLUMNS: [&str; 12] = [
    "compound_qid",
    "compound_name",
    "inchikey",
    "smiles",
    "mass",
    "formula",
    "taxon_qid",
    "taxon_name",
    "reference_qid",
    "reference_title",
    "reference_doi",
    "pub_year",
];

/// How a result set is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// Aligned columns for a terminal. Not for parsing.
    Table,
    /// Tab-separated, one header row.
    Tsv,
    /// Comma-separated, RFC 4180 quoted.
    Csv,
    /// A single JSON object, for a result set of any size.
    Json,
    /// One JSON object per line, for streaming into another process.
    Jsonl,
    /// JSON-LD, following the Bioschemas profiles.
    Jsonld,
    /// The SPARQL that produced the rows.
    Query,
}

/// Write a result set in `format`.
///
/// # Errors
/// Returns an error if the writer fails, which for stdout means a closed pipe —
/// `lotus search … | head` is a normal thing to do and the error is worth
/// reporting rather than panicking on.
pub fn write_rows<W: Write>(
    mut out: W,
    result: &SearchResult,
    format: Format,
    quiet: bool,
) -> anyhow::Result<()> {
    match format {
        Format::Table => write_table(&mut out, &result.rows)?,
        Format::Tsv => write_delimited(&mut out, &result.rows, b'\t')?,
        Format::Csv => write_delimited(&mut out, &result.rows, b',')?,
        Format::Json => write_json(&mut out, result)?,
        Format::Jsonl => write_jsonl(&mut out, &result.rows)?,
        Format::Jsonld => write_jsonld(&mut out, result)?,
        Format::Query => writeln!(out, "{}", result.query)?,
    }
    if !quiet {
        out.flush()?;
    }
    Ok(())
}

fn cell(entry: &CompoundEntry, column: &str) -> String {
    match column {
        "compound_qid" => entry.compound_qid.to_string(),
        "compound_name" => entry.name.to_string(),
        "inchikey" => entry.inchikey.as_deref().unwrap_or_default().to_string(),
        "smiles" => entry.smiles.as_deref().unwrap_or_default().to_string(),
        "mass" => entry.mass.map_or_else(String::new, |m| format!("{m}")),
        "formula" => entry.formula.as_deref().unwrap_or_default().to_string(),
        "taxon_qid" => entry.taxon_qid.to_string(),
        "taxon_name" => entry.taxon_name.to_string(),
        "reference_qid" => entry.reference_qid.to_string(),
        "reference_title" => entry.ref_title.as_deref().unwrap_or_default().to_string(),
        "reference_doi" => entry.ref_doi.as_deref().unwrap_or_default().to_string(),
        "pub_year" => entry.pub_year.map_or_else(String::new, |y| y.to_string()),
        _ => String::new(),
    }
}

fn write_delimited<W: Write>(out: &mut W, rows: &[CompoundEntry], sep: u8) -> anyhow::Result<()> {
    let header: Vec<String> = COLUMNS.iter().map(|c| (*c).to_string()).collect();
    out.write_all(join(&header, sep).as_bytes())?;
    out.write_all(b"\n")?;
    for row in rows {
        let line: Vec<String> = COLUMNS.iter().map(|c| cell(row, c)).collect();
        out.write_all(join(&line, sep).as_bytes())?;
        out.write_all(b"\n")?;
    }
    Ok(())
}

/// Join and quote, so that a value containing the separator cannot break the
/// file. CSV follows RFC 4180; TSV gets the same treatment because a
/// `tab`-containing field is just as corrupting.
fn join(values: &[String], sep: u8) -> String {
    values
        .iter()
        .map(|value| quote(value, sep))
        .collect::<Vec<_>>()
        .join(&(sep as char).to_string())
}

fn quote(value: &str, sep: u8) -> String {
    let needs_quoting = value.contains(sep as char)
        || value.contains('"')
        || value.contains('\n')
        || value.contains('\r');
    if needs_quoting {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// The columns a table shows. A structure column is the widest thing in a
/// result set and the least useful at a glance, so it is left out here and
/// kept in the machine-readable formats.
const SHOWN: [&str; 10] = [
    "compound_qid",
    "compound_name",
    "formula",
    "mass",
    "taxon_name",
    "reference_doi",
    "pub_year",
    "inchikey",
    "reference_title",
    "statement",
];

/// A table, aligned by the widest value per column.
fn write_table<W: Write>(out: &mut W, rows: &[CompoundEntry]) -> anyhow::Result<()> {
    if rows.is_empty() {
        writeln!(out, "no matches")?;
        return Ok(());
    }

    let header: Vec<String> = SHOWN.iter().map(|c| (*c).to_string()).collect();
    let body: Vec<Vec<String>> = rows
        .iter()
        .map(|row| SHOWN.iter().map(|c| cell(row, c)).collect())
        .collect();

    let mut widths: Vec<usize> = header.iter().map(String::len).collect();
    for line in &body {
        for (width, value) in widths.iter_mut().zip(line) {
            *width = (*width).max(value.chars().count());
        }
    }

    // The last column is not padded, so a line has no trailing whitespace.
    let render = |values: &[String]| -> String {
        values
            .iter()
            .enumerate()
            .map(|(i, value)| match widths.get(i) {
                Some(width) if i + 1 < values.len() => format!("{value:<width$}"),
                _ => value.clone(),
            })
            .collect::<Vec<_>>()
            .join("  ")
    };

    writeln!(out, "{}", render(&header))?;
    for line in &body {
        writeln!(out, "{}", render(line))?;
    }
    writeln!(out, "\n{} row(s)", rows.len())?;
    Ok(())
}

fn write_json<W: Write>(out: &mut W, result: &SearchResult) -> anyhow::Result<()> {
    let value = serde_json::json!({
        "query": result.query,
        "rows": result.rows,
        "stats": result.stats,
        "truncated": result.truncated,
    });
    serde_json::to_writer_pretty(&mut *out, &value)?;
    writeln!(out)?;
    Ok(())
}

fn write_jsonl<W: Write>(out: &mut W, rows: &[CompoundEntry]) -> anyhow::Result<()> {
    for row in rows {
        serde_json::to_writer(&mut *out, row)?;
        writeln!(out)?;
    }
    Ok(())
}

fn write_jsonld<W: Write>(out: &mut W, result: &SearchResult) -> anyhow::Result<()> {
    let generated = format!("{}-01-01T00:00:00Z", crate::current_year());
    let query_hash = hash(&result.query);
    let result_hash = hash(&serde_json::to_string(&result.rows)?);

    let document = serde_json::json!({
        "@context": "https://schema.org/",
        "@graph": [
            lotus_jsonld::result_set_jsonld(&lotus_jsonld::ResultSet {
                query: &result.query,
                taxon: result
                    .taxon
                    .as_ref()
                    .and_then(lotus_search::TaxonResolution::looked_up_name)
                    .unwrap_or_default(),
                query_hash: &query_hash,
                result_hash: &result_hash,
                total_entries: result
                    .stats
                    .as_ref()
                    .map_or(result.rows.len(), |s| s.n_entries),
                generated: &generated,
            }),
            lotus_jsonld::dataset_jsonld(),
        ],
        "mainEntity": result
            .rows
            .iter()
            .map(lotus_jsonld::compound_jsonld)
            .filter(|v| !v.is_null())
            .collect::<Vec<_>>(),
    });

    serde_json::to_writer_pretty(&mut *out, &document)?;
    writeln!(out)?;
    Ok(())
}

fn hash(value: &str) -> String {
    use std::fmt::Write as _;
    let digest = sha2::Sha256::digest(value.as_bytes());
    digest
        .iter()
        .fold(String::with_capacity(64), |mut acc, byte| {
            let _ = write!(acc, "{byte:02x}");
            acc
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;
    use std::sync::Arc;

    fn entry() -> CompoundEntry {
        CompoundEntry {
            compound_qid: Arc::from("Q16521"),
            name: Arc::from("Quercetin"),
            formula: Some(Arc::from("C15H10O7")),
            mass: Some(302.24),
            taxon_name: Arc::from("Gentiana lutea"),
            ..CompoundEntry::default()
        }
    }

    fn render(
        f: impl Fn(&mut Vec<u8>, &[CompoundEntry]) -> anyhow::Result<()>,
        rows: &[CompoundEntry],
    ) -> String {
        let mut out = Vec::new();
        f(&mut out, rows).expect("writing to a Vec cannot fail");
        String::from_utf8(out).expect("output is UTF-8 by construction")
    }

    #[test]
    fn a_value_containing_the_separator_is_quoted_so_the_file_still_parses() {
        // The whole point of quoting: a taxon name with a comma in it would
        // otherwise split into two columns and every column after it shifts.
        assert_eq!(quote("a,b", b','), r#""a,b""#);
        assert_eq!(quote("a\tb", b'\t'), "\"a\tb\"");
    }

    #[test]
    fn a_quote_inside_a_value_is_doubled_not_dropped() {
        // RFC 4180. A single `"` would end the field early and the rest of the
        // value would be read as the next field.
        assert_eq!(quote(r#"say "hi""#, b','), r#""say ""hi""""#);
    }

    #[test]
    fn a_newline_inside_a_value_is_quoted() {
        assert_eq!(quote("two\nlines", b','), "\"two\nlines\"");
        assert_eq!(quote("two\r\nlines", b','), "\"two\r\nlines\"");
    }

    #[test]
    fn an_ordinary_value_is_left_alone() {
        // Quoting everything is valid CSV but not valid TSV, where a consumer
        // that splits on tabs would keep the quotes.
        for plain in ["Gentiana lutea", "302.24", "C15H10O7", ""] {
            assert_eq!(quote(plain, b','), plain, "{plain:?}");
            assert_eq!(quote(plain, b'\t'), plain, "{plain:?}");
        }
    }

    #[test]
    fn the_csv_header_names_every_column() {
        let out = render(|o, r| write_delimited(o, r, b','), &[entry()]);
        let header = out.lines().next().expect("a header line");
        for column in COLUMNS {
            assert!(
                header.contains(column),
                "{column:?} missing from {header:?}"
            );
        }
    }

    #[test]
    fn a_csv_row_carries_the_values_the_query_returned() {
        let out = render(|o, r| write_delimited(o, r, b','), &[entry()]);
        let row = out.lines().nth(1).expect("one data row");
        assert!(row.contains("Q16521"), "{row}");
        assert!(row.contains("Quercetin"), "{row}");
        assert!(row.contains("Gentiana lutea"), "{row}");
    }

    #[test]
    fn tsv_is_delimited_by_tabs_and_not_commas() {
        // A TSV whose cells contain commas must not gain quote characters, or
        // the first column of every row arrives wrapped in them.
        let out = render(|o, r| write_delimited(o, r, b'\t'), &[entry()]);
        let header = out.lines().next().expect("a header line");
        assert!(header.contains('\t'), "{header:?}");
        assert!(
            !header.contains('"'),
            "TSV headers are not quoted: {header:?}"
        );
    }

    #[test]
    fn an_empty_result_set_says_so_rather_than_printing_an_empty_table() {
        // The default format. A header with no rows reads as a broken query.
        assert_eq!(render(write_table, &[]).trim(), "no matches");
    }

    #[test]
    fn a_table_column_is_as_wide_as_its_widest_value() {
        // Alignment is the reason this format exists, so it is worth pinning:
        // two spaces between columns, and the next column starts past the
        // longest cell above it.
        let out = render(write_table, &[entry()]);
        let mut lines = out.lines();
        let header = lines.next().expect("a header line");
        let row = lines.next().expect("a data row");
        let column = header.find("formula").expect("the formula column");
        assert_eq!(row.as_bytes()[column], b'C', "misaligned: {row:?}");
    }

    #[test]
    fn an_absent_optional_value_renders_as_nothing_rather_than_null() {
        // A row with no DOI must not print "None" or "null" into a column a
        // person is about to read.
        let out = render(write_table, &[entry()]);
        assert!(!out.contains("None"), "{out}");
        assert!(!out.contains("null"), "{out}");
    }
}
