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
    // Every value is padded to its column and the line is then trimmed, which is
    // simpler than special-casing the last one: padding it and trimming it off
    // gives the same string. Not padding the last value is not enough on its own
    // -- the separator before it is still joined in, so an empty last column
    // leaves the line ending in spaces, and that shows up in every diff of the
    // output and in every `cat -A`.
    let render = |values: &[String]| -> String {
        values
            .iter()
            .enumerate()
            .map(|(i, value)| {
                widths
                    .get(i)
                    .map_or_else(|| value.clone(), |width| format!("{value:<width$}"))
            })
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_owned()
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
    // The panic lints keep shipped code free of panics on external input. A test
    // that fails on a malformed fixture is reporting, not panicking.
    #![allow(clippy::expect_used, clippy::panic)]

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
    fn a_table_column_starts_where_the_header_above_it_does() {
        // Alignment is the reason this format exists, so it is worth pinning: the
        // next column begins past the widest cell above it. Compared by byte
        // position rather than by slicing, so a short row reports a mismatch
        // instead of panicking.
        let out = render(write_table, &[entry()]);
        let mut lines = out.lines();
        let header = lines.next().expect("a header line");
        let row = lines.next().expect("a data row");
        let column = header.find("formula").expect("the formula column");

        let Some(at) = row.as_bytes().get(column).copied() else {
            panic!("row is shorter than the header: header {header:?} row {row:?}");
        };
        assert_eq!(at, b'C', "misaligned: header {header:?} row {row:?}");
    }

    #[test]
    fn an_absent_optional_value_renders_as_nothing_rather_than_null() {
        // A row with no DOI must not print "None" or "null" into a column a
        // person is about to read.
        let out = render(write_table, &[entry()]);
        assert!(!out.contains("None"), "{out}");
        assert!(!out.contains("null"), "{out}");
    }

    /// A row with every optional field filled in, so a column that reads the
    /// wrong field is visible rather than indistinguishable from an absent one.
    fn full_entry() -> CompoundEntry {
        CompoundEntry {
            compound_qid: Arc::from("Q1234"),
            name: Arc::from("Quercetin"),
            inchikey: Some(Arc::from("IIYFPWUAQGXNF-FGGY-VCPS-NA-NSA")),
            smiles: Some(Arc::from("O=c1c(O)c(-c2ccc(O)c(O)c2)oc2cc(O)cc(O)c12")),
            mass: Some(302.24),
            formula: Some(Arc::from("C15H10O7")),
            taxon_qid: Arc::from("Q9999"),
            taxon_name: Arc::from("Gentiana lutea"),
            reference_qid: Arc::from("Q8888"),
            ref_title: Some(Arc::from("A paper")),
            ref_doi: Some(Arc::from("10.1000/paper")),
            pub_year: Some(2021),
            statement: None,
        }
    }

    #[test]
    fn every_column_reads_the_field_of_that_name() {
        // `cell` is a lookup by column name, so a mistyped arm returns an empty
        // cell rather than failing: a `--columns` typo gave a table full of
        // blanks and no error. Each arm therefore asserts the value it is
        // supposed to read.
        let entry = full_entry();
        for (column, expected) in [
            ("compound_qid", "Q1234"),
            ("compound_name", "Quercetin"),
            ("inchikey", "IIYFPWUAQGXNF-FGGY-VCPS-NA-NSA"),
            ("smiles", "O=c1c(O)c(-c2ccc(O)c(O)c2)oc2cc(O)cc(O)c12"),
            ("formula", "C15H10O7"),
            ("taxon_qid", "Q9999"),
            ("taxon_name", "Gentiana lutea"),
            ("reference_qid", "Q8888"),
            ("reference_title", "A paper"),
            ("reference_doi", "10.1000/paper"),
            ("pub_year", "2021"),
        ] {
            assert_eq!(cell(&entry, column), expected, "column {column}");
        }
        // A mass is a float and renders through `Display`, so it has to come out
        // spelled the way the endpoint spelled it rather than rounded.
        assert_eq!(cell(&entry, "mass"), "302.24");
    }

    #[test]
    fn an_unknown_column_is_empty_rather_than_an_error() {
        // The name is a command-line value, so a typo is a blank column, not a
        // failure -- which is why each real column needs its own test above.
        assert_eq!(cell(&full_entry(), "nonsense"), "");
    }

    #[test]
    fn an_absent_optional_value_is_an_empty_cell() {
        // Every optional field absent at once. `None` rendered as "null" or
        // "None" would be read as data by whatever consumes the table.
        let bare = CompoundEntry {
            compound_qid: Arc::from("Q1"),
            ..CompoundEntry::default()
        };
        for column in [
            "inchikey",
            "smiles",
            "mass",
            "formula",
            "reference_title",
            "reference_doi",
            "pub_year",
        ] {
            assert_eq!(
                cell(&bare, column),
                "",
                "column {column} of an absent value"
            );
        }
        // The two that are not optional still read.
        assert_eq!(cell(&bare, "compound_qid"), "Q1");
        assert_eq!(cell(&bare, "compound_name"), "");
    }

    #[test]
    fn no_line_of_a_table_ends_in_whitespace() {
        // Padding the final cell leaves trailing whitespace on every row, which
        // breaks a line-oriented diff of the output and reads as a defect in a
        // file that is otherwise fine.
        let mut out = Vec::new();
        write_table(&mut out, &[full_entry(), entry()]).expect("a table");
        let table = String::from_utf8(out).expect("utf-8");
        assert!(
            table.lines().filter(|line| !line.trim().is_empty()).count() >= 3,
            "a header and two rows:\n{table}"
        );
        for line in table.lines() {
            assert_eq!(line, line.trim_end(), "trailing whitespace in {line:?}");
        }
    }

    #[test]
    fn a_table_line_ends_where_the_value_ends() {
        // The column widths come from the values, so a row whose last value is
        // the longest has no padding and a row whose is shorter is still one
        // line. Both matter: a wrapped row is read as two rows.
        let mut out = Vec::new();
        write_table(&mut out, &[full_entry(), entry()]).expect("a table");
        let table = String::from_utf8(out).expect("utf-8");
        // Header, rule, then one line per row.
        let rows: Vec<&str> = table
            .lines()
            .filter(|line| line.contains("Quercetin"))
            .collect();
        let [first, second] = rows.as_slice() else {
            panic!("one line per row:\n{table}");
        };
        assert!(first.contains("Q1234"), "got {first:?}");
        assert!(second.contains("Q16521"), "got {second:?}");
    }

    /// A writer that fails every operation, so the error paths are reachable
    /// without a real disk or socket.
    struct Failing;

    impl std::io::Write for Failing {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("the pipe closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("the pipe closed"))
        }
    }

    /// A writer that accepts everything and then fails to flush.
    ///
    /// Distinct from [`Failing`] because the two failures are separate: a run
    /// that buffers its whole output can write successfully and still fail to
    /// hand it over, and only a writer like this can tell the two apart.
    struct FailsToFlush(Vec<u8>);

    impl std::io::Write for FailsToFlush {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("the pipe closed on flush"))
        }
    }

    const FORMATS: [Format; 7] = [
        Format::Table,
        Format::Tsv,
        Format::Csv,
        Format::Json,
        Format::Jsonl,
        Format::Jsonld,
        Format::Query,
    ];

    fn result() -> SearchResult {
        SearchResult {
            rows: vec![full_entry()],
            stats: None,
            taxon: None,
            query: "SELECT ?compound WHERE { ?compound wdt:P31 wd:Q11388 . }".to_owned(),
            truncated: false,
        }
    }

    #[test]
    fn every_format_reports_a_failing_writer_instead_of_dropping_it() {
        // A write error swallowed here is a search that appeared to succeed and
        // produced a truncated file, which is the outcome nobody checks for.
        for format in FORMATS {
            assert!(
                write_rows(Failing, &result(), format, false).is_err(),
                "{format:?} reported success on a closed pipe"
            );
        }
    }

    #[test]
    fn every_format_writes_something_for_a_result_with_a_row() {
        // The other half of the same thing: a format whose writer returns early
        // without writing produces no error either, so a run looks successful and
        // the file is empty. Asserting the bytes are there catches that where an
        // error check cannot.
        for format in FORMATS {
            let mut out = Vec::new();
            write_rows(&mut out, &result(), format, false).expect("writing to a Vec");
            let written = String::from_utf8(out).expect("utf-8");
            assert!(!written.trim().is_empty(), "{format:?} wrote nothing");
            if format != Format::Query {
                // Every other format is rows. `--query` is the one that writes
                // the SPARQL instead, and the test above already covers it.
                assert!(
                    written.contains("Q1234") || written.contains("Quercetin"),
                    "{format:?} wrote no row: {written:?}"
                );
            }
        }
    }

    #[test]
    fn a_quiet_run_does_not_flush() {
        // `--quiet` exists so the data goes to a file or a pipe without a summary
        // being appended. Flushing is part of that: a quiet run that flushed
        // would still be trying to hand something over, and would fail on a
        // writer that only fails on flush.
        let quiet = FailsToFlush(Vec::new());
        assert!(
            write_rows(quiet, &result(), Format::Csv, true).is_ok(),
            "a quiet run must not flush"
        );
    }

    #[test]
    fn a_loud_run_flushes_and_reports_that_it_could_not() {
        // The counterpart: the same writer, not quiet. The output was written and
        // buffered, and losing it is an error the caller has to hear about.
        let loud = FailsToFlush(Vec::new());
        assert!(
            write_rows(loud, &result(), Format::Csv, false).is_err(),
            "a flush failure has to surface, not vanish into a buffer"
        );
    }

    #[test]
    fn a_quiet_run_that_writes_nothing_still_reports_a_write_failure() {
        // `--quiet` suppresses the flush, not the writing. A quiet run that
        // swallowed the error would be quiet about failing too.
        for format in FORMATS {
            assert!(
                write_rows(Failing, &result(), format, true).is_err(),
                "{format:?} swallowed a write failure while quiet"
            );
        }
    }

    #[test]
    fn a_quiet_run_writes_the_data_and_nothing_else() {
        // The point of `--quiet`: the rows go out and no summary is appended.
        let mut out = Vec::new();
        write_rows(&mut out, &result(), Format::Query, true).expect("quiet");
        assert_eq!(
            String::from_utf8(out).expect("utf-8"),
            "SELECT ?compound WHERE { ?compound wdt:P31 wd:Q11388 . }\n",
        );
    }

    #[test]
    fn the_dataset_hash_is_stable_and_says_something() {
        // It is part of a dataset URL, so it has to be a function of the name
        // and the query and nothing else. An empty or constant hash would put
        // two different result sets at the same address.
        let first = super::hash("quercetin|SELECT 1");
        assert_eq!(
            first,
            super::hash("quercetin|SELECT 1"),
            "the same input, the same hash"
        );
        assert_eq!(first.len(), 64, "a full sha-256 in hex");
        assert!(
            first
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "lowercase hex, got {first:?}"
        );
        assert_ne!(
            first,
            super::hash("quercetin|SELECT 2"),
            "the query is part of it"
        );
        assert_ne!(
            first,
            super::hash("rutin|SELECT 1"),
            "the name is part of it"
        );
    }
}
