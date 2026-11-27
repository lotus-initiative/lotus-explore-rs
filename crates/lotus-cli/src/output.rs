// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Writing a result set to stdout.
//!
//! Data goes to stdout, diagnostics to stderr, so that `lotus search … > out.csv`
//! captures the data and nothing else.

use std::borrow::Cow;
use std::fmt::Write as _;
use std::io::Write;

use clap::ValueEnum;
use lotus_model::CompoundEntry;
use lotus_search::SearchResult;
use sha2::Digest;

#[derive(Clone, Copy)]
enum Column {
    CompoundQid,
    CompoundName,
    Inchikey,
    Smiles,
    Mass,
    Formula,
    TaxonQid,
    TaxonName,
    ReferenceQid,
    ReferenceTitle,
    ReferenceDoi,
    PubYear,
    Statement,
}

impl Column {
    const fn header(self) -> &'static str {
        match self {
            Self::CompoundQid => "compound_qid",
            Self::CompoundName => "compound_name",
            Self::Inchikey => "inchikey",
            Self::Smiles => "smiles",
            Self::Mass => "mass",
            Self::Formula => "formula",
            Self::TaxonQid => "taxon_qid",
            Self::TaxonName => "taxon_name",
            Self::ReferenceQid => "reference_qid",
            Self::ReferenceTitle => "reference_title",
            Self::ReferenceDoi => "reference_doi",
            Self::PubYear => "pub_year",
            Self::Statement => "statement",
        }
    }

    fn value(self, entry: &CompoundEntry) -> Cow<'_, str> {
        match self {
            Self::CompoundQid => Cow::Borrowed(&entry.compound_qid),
            Self::CompoundName => Cow::Borrowed(&entry.name),
            Self::Inchikey => Cow::Borrowed(entry.inchikey.as_deref().unwrap_or_default()),
            Self::Smiles => Cow::Borrowed(entry.smiles.as_deref().unwrap_or_default()),
            Self::Mass => entry
                .mass
                .map_or(Cow::Borrowed(""), |m| Cow::Owned(format!("{m}"))),
            Self::Formula => Cow::Borrowed(entry.formula.as_deref().unwrap_or_default()),
            Self::TaxonQid => Cow::Borrowed(&entry.taxon_qid),
            Self::TaxonName => Cow::Borrowed(&entry.taxon_name),
            Self::ReferenceQid => Cow::Borrowed(&entry.reference_qid),
            Self::ReferenceTitle => Cow::Borrowed(entry.ref_title.as_deref().unwrap_or_default()),
            Self::ReferenceDoi => Cow::Borrowed(entry.ref_doi.as_deref().unwrap_or_default()),
            Self::PubYear => entry
                .pub_year
                .map_or(Cow::Borrowed(""), |y| Cow::Owned(y.to_string())),
            Self::Statement => Cow::Borrowed(entry.statement.as_deref().unwrap_or_default()),
        }
    }

    #[cfg(test)]
    fn parse(name: &str) -> Option<Self> {
        match name {
            "compound_qid" => Some(Self::CompoundQid),
            "compound_name" => Some(Self::CompoundName),
            "inchikey" => Some(Self::Inchikey),
            "smiles" => Some(Self::Smiles),
            "mass" => Some(Self::Mass),
            "formula" => Some(Self::Formula),
            "taxon_qid" => Some(Self::TaxonQid),
            "taxon_name" => Some(Self::TaxonName),
            "reference_qid" => Some(Self::ReferenceQid),
            "reference_title" => Some(Self::ReferenceTitle),
            "reference_doi" => Some(Self::ReferenceDoi),
            "pub_year" => Some(Self::PubYear),
            "statement" => Some(Self::Statement),
            _ => None,
        }
    }
}

/// The columns a table or a delimited file carries, in order.
const COLUMNS: [Column; 12] = [
    Column::CompoundQid,
    Column::CompoundName,
    Column::Inchikey,
    Column::Smiles,
    Column::Mass,
    Column::Formula,
    Column::TaxonQid,
    Column::TaxonName,
    Column::ReferenceQid,
    Column::ReferenceTitle,
    Column::ReferenceDoi,
    Column::PubYear,
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
    /// RDF Turtle, for ingestion into a triple store.
    ///
    /// Rendered from the fetched rows rather than by asking the endpoint to run
    /// the `CONSTRUCT`, because the rows are already here: a second request would
    /// fetch the same result again just to change its syntax. `ttl` and `turtle`
    /// are accepted as aliases, which is what anyone typing them expects.
    #[value(alias = "ttl", alias = "turtle")]
    Rdf,
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
        Format::Rdf => write_rdf(&mut out, result)?,
        Format::Query => writeln!(out, "{}", result.query)?,
    }
    if !quiet {
        out.flush()?;
    }
    Ok(())
}

#[cfg(test)]
fn cell<'a>(entry: &'a CompoundEntry, column: &str) -> Cow<'a, str> {
    Column::parse(column).map_or(Cow::Borrowed(""), |col| col.value(entry))
}

fn write_delimited<W: Write>(out: &mut W, rows: &[CompoundEntry], sep: u8) -> anyhow::Result<()> {
    let mut buf = String::new();
    join_into(&mut buf, COLUMNS.map(Column::header), sep);
    out.write_all(buf.as_bytes())?;
    for row in rows {
        buf.clear();
        join_into(&mut buf, COLUMNS.map(|c| c.value(row)), sep);
        out.write_all(buf.as_bytes())?;
    }
    Ok(())
}

/// Join and quote, so that a value containing the separator cannot break the
/// file. CSV follows RFC 4180; TSV gets the same treatment because a
/// `tab`-containing field is just as corrupting.
fn join_into<I, S>(buf: &mut String, values: I, sep: u8)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut iter = values.into_iter();
    if let Some(first) = iter.next() {
        buf.push_str(&quote(first.as_ref(), sep));
        for value in iter {
            buf.push(sep as char);
            buf.push_str(&quote(value.as_ref(), sep));
        }
    }
    buf.push('\n');
}

fn quote(value: &str, sep: u8) -> Cow<'_, str> {
    let needs_quoting = value.contains(sep as char)
        || value.contains('"')
        || value.contains('\n')
        || value.contains('\r');
    if needs_quoting {
        Cow::Owned(format!("\"{}\"", value.replace('"', "\"\"")))
    } else {
        Cow::Borrowed(value)
    }
}

/// The columns a table shows. A structure column is the widest thing in a
/// result set and the least useful at a glance, so it is left out here and
/// kept in the machine-readable formats.
const SHOWN: [Column; 10] = [
    Column::CompoundQid,
    Column::CompoundName,
    Column::Formula,
    Column::Mass,
    Column::TaxonName,
    Column::ReferenceDoi,
    Column::PubYear,
    Column::Inchikey,
    Column::ReferenceTitle,
    Column::Statement,
];

/// A table, aligned by the widest value per column.
fn write_table<W: Write>(out: &mut W, rows: &[CompoundEntry]) -> anyhow::Result<()> {
    if rows.is_empty() {
        writeln!(out, "no matches")?;
        return Ok(());
    }

    let body: Vec<[Cow<'_, str>; SHOWN.len()]> =
        rows.iter().map(|row| SHOWN.map(|c| c.value(row))).collect();

    let headers = SHOWN.map(Column::header);
    let mut widths: [usize; SHOWN.len()] = headers.map(str::len);
    for line in &body {
        for (width, value) in widths.iter_mut().zip(line) {
            *width = (*width).max(value.chars().count());
        }
    }

    // The last column is not padded, so a line has no trailing whitespace. Every value
    // is padded and the line then trimmed, which is simpler than special-casing
    // the last one. Not padding the last value is not enough on its own: the
    // separator before it is still joined in, so an empty last column leaves the
    // line ending in spaces, visible in every diff and in every `cat -A`.
    let render = |values: &[&str]| -> String {
        let mut line = String::new();
        for (value, width) in values.iter().zip(widths) {
            let _ = write!(line, "{value:<width$}  ");
        }
        line.truncate(line.trim_end().len());
        line
    };

    writeln!(out, "{}", render(&headers))?;
    for line in &body {
        let refs = line.each_ref().map(Cow::as_ref);
        writeln!(out, "{}", render(&refs))?;
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

/// Write the result set as Turtle, one chunk at a time.
///
/// Chunked for the same reason the row exporters are: the set is already fully
/// built, so streaming the chunks keeps a large export from also building a second
/// copy of it as one string.
fn write_rdf<W: Write>(out: &mut W, result: &SearchResult) -> anyhow::Result<()> {
    use lotus_query::{ExportFormat, RowExporter};

    let set = lotus_model::ColumnarResultSet::from_entries(&result.rows);
    let mut exporter = RowExporter::new(ExportFormat::Rdf, &set);
    while let Some(chunk) = exporter.next_chunk() {
        out.write_all(chunk.as_bytes())?;
    }
    Ok(())
}

fn write_jsonld<W: Write>(out: &mut W, result: &SearchResult) -> anyhow::Result<()> {
    let generated = crate::now_iso_utc();
    write_jsonld_at(out, result, &generated)
}

fn write_jsonld_at<W: Write>(
    out: &mut W,
    result: &SearchResult,
    generated: &str,
) -> anyhow::Result<()> {
    let query_hash = hash(&result.query);
    let result_hash = hash(&serde_json::to_string(&result.rows)?);

    let mut graph = vec![
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
            generated,
        }),
        lotus_jsonld::dataset_jsonld(),
    ];
    graph.extend(
        result
            .rows
            .iter()
            .map(lotus_jsonld::compound_jsonld)
            .filter(|v| !v.is_null()),
    );

    let document = serde_json::json!({
        "@context": lotus_jsonld::jsonld_context(),
        "@graph": graph,
    });

    serde_json::to_writer_pretty(&mut *out, &document)?;
    writeln!(out)?;
    Ok(())
}

fn hash(value: &str) -> String {
    lotus_model::encode_hex_lower(&sha2::Sha256::digest(value.as_bytes()))
}

#[cfg(test)]
#[path = "output/tests.rs"]
mod tests;
