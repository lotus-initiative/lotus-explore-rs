// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `lotus curate` — check a TSV of findings against Wikidata.
//!
//! Nothing here writes to Wikidata. The output is a set of `QuickStatements` that
//! a curator reviews and submits, and there is no flag that submits for them:
//! running the command twice cannot double-submit anything, because running it
//! submits nothing.

use std::io::{self, Read as _, Write};
use std::process::ExitCode;

use clap::ValueEnum;

/// What `curate` does with a set of rows.
#[derive(Debug, clap::Args)]
pub struct CurateArgs {
    /// TSV with `name` and `smiles` columns, and optionally `taxon` and `doi`.
    /// Reads stdin when omitted or `-`.
    #[arg(value_name = "TSV")]
    pub input: Option<String>,

    /// Write the `QuickStatements` here rather than to stdout.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<std::path::PathBuf>,

    /// How the report is written.
    #[arg(long, value_enum, default_value_t = ReportFormat::Table)]
    pub format: ReportFormat,

    /// Suppress the reminder on stderr that the statements have not been
    /// submitted.
    ///
    /// Nothing is ever submitted. This only silences the reminder, for when the
    /// output is being piped somewhere that reads stderr as noise. There is
    /// deliberately no flag that submits: writing to Wikidata is a decision
    /// with a person attached to it, and a batch of a few hundred statements
    /// arriving from a shell script at 3am is not one.
    #[arg(long)]
    pub quiet: bool,

    /// Log to stderr: error, warn, info, debug.
    #[arg(long, value_enum, default_value_t = LogLevel::Error)]
    pub log: LogLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ReportFormat {
    Table,
    Tsv,
    Json,
    Jsonl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

/// One row of the input file.
///
/// The curation vocabulary lives in `lotus-curation`, because the web client
/// curates from the same file and the two used to parse it differently.
pub use lotus_curation::CurationInputRow as Finding;

/// Parse a TSV by column name.
///
/// Columns are matched by name rather than position, because an export from a
/// spreadsheet carries whatever else it has and a caller should not have to
/// strip it first.
pub fn parse_tsv(tsv: &str) -> Result<Vec<Finding>, String> {
    lotus_curation::parse_tsv(tsv).map_err(|e| e.to_string())
}

/// The two findings that name the same compound in the same taxon with the same
/// reference are the same finding, whatever they were called.
#[must_use]
pub fn identity_key(finding: &Finding) -> String {
    lotus_curation::row_uniqueness_key(finding)
}

pub fn run(args: &CurateArgs) -> anyhow::Result<ExitCode> {
    let input = match args.input.as_deref() {
        None | Some("-") => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        }
        Some(path) => std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("could not read {path}: {e}"))?,
    };

    let findings = parse_tsv(&input).map_err(|e| anyhow::anyhow!("{e}"))?;
    if findings.is_empty() {
        eprintln!("lotus: no usable rows in the input");
        return Ok(ExitCode::FAILURE);
    }

    // A duplicate is a row twice in the same file, which is a spreadsheet
    // accident rather than two findings.
    let mut seen = std::collections::HashSet::new();
    let total = findings.len();
    let unique: Vec<Finding> = findings
        .into_iter()
        .filter(|f| seen.insert(identity_key(f)))
        .collect();
    let duplicates = total - unique.len();
    if duplicates > 0 {
        eprintln!("lotus: {duplicates} duplicate row(s) ignored");
    }

    if !args.quiet {
        eprintln!(
            "lotus: nothing has been submitted. These are statements for a \
             person to read and submit."
        );
    }

    let report = build_report(&unique);
    if let Some(path) = &args.output {
        let mut file = std::fs::File::create(path)
            .map_err(|e| anyhow::anyhow!("could not write {}: {e}", path.display()))?;
        write_report(&mut file, &report, args.format)?;
    } else {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        write_report(&mut handle, &report, args.format)?;
        handle.flush()?;
    }

    Ok(ExitCode::SUCCESS)
}

/// What curation would do to each row.
///
/// Every row is reported as new. The command does not query Wikidata, so it
/// cannot know whether an item already exists, and saying "new" about something
/// that is already there is the less harmful of the two wrong answers: a
/// curator reads a "new" label and looks, and reads an "exists" label and does
/// not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub findings: Vec<Finding>,
    /// `QuickStatements` a curator would submit, in dependency order.
    pub statements: Vec<String>,
    /// The LOTUS paper, which every result should cite.
    pub citation: &'static str,
}

/// Escape a value for a `QuickStatements` scalar.
///
/// The format is pipe-separated with `"`-quoted scalars, and a value containing
/// a quote or a newline will otherwise end the scalar early and write into the
/// next line as though it were a new property.
fn escape_quickstatements(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\n' | '\r' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// The properties a LOTUS row is written against, from the Wikidata projection.
///
/// `P233` is the canonical SMILES and `P2017` the isomeric one. Both are emitted
/// because the explorer queries both: a row with only `P233` matches a
/// structure search by canonical form and not by the form the chemist submitted.
const PROPERTY_CANONICAL_SMILES: &str = "P233";
const PROPERTY_ISOMERIC_SMILES: &str = "P2017";
const PROPERTY_NAME: &str = "Len";
/// `P703` — "found in taxon", the occurrence link.
const PROPERTY_OCCURS_IN_TAXON: &str = "P703";
/// The reference link, a `prov:wasDerivedFrom` reference block.
const PROPERTY_STATED_IN: &str = "P248";

fn build_report(findings: &[Finding]) -> Report {
    use std::fmt::Write as _;

    let mut statements = Vec::new();
    for finding in findings {
        let mut block = format!(
            "## {}\nCREATE\nLAST|{PROPERTY_NAME}|\"{}\"\nLAST|{PROPERTY_CANONICAL_SMILES}|\"{}\"\nLAST|{PROPERTY_ISOMERIC_SMILES}|\"{}\"",
            finding.name,
            escape_quickstatements(&finding.name),
            escape_quickstatements(finding.smiles.trim()),
            escape_quickstatements(finding.smiles.trim()),
        );
        // The occurrence and the reference are separate statements because they
        // need items that may not exist yet. A `QuickStatements` run stops at the
        // first failure, so an occurrence pointing at a taxon nobody has created
        // would take the whole block down -- including the compound itself,
        // which is the part a curator most wants.
        if let Some(taxon) = finding
            .taxon
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            let _ = writeln!(
                block,
                "\n## {name} — occurrence\nCREATE\nLAST|{PROPERTY_OCCURS_IN_TAXON}|\"{taxon}\"",
                name = escape_quickstatements(&finding.name),
                taxon = escape_quickstatements(taxon),
            );
        }
        if let Some(doi) = finding
            .doi
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty())
        {
            let _ = writeln!(
                block,
                "\n## {name} — reference\nCREATE\nLAST|{PROPERTY_STATED_IN}|\"{doi}\"",
                name = escape_quickstatements(&finding.name),
                doi = escape_quickstatements(doi),
            );
        }
        statements.push(block);
    }
    Report {
        findings: findings.to_vec(),
        statements,
        citation: "https://doi.org/10.7554/eLife.70780",
    }
}

fn write_report<W: Write>(
    out: &mut W,
    report: &Report,
    format: ReportFormat,
) -> anyhow::Result<()> {
    match format {
        ReportFormat::Table => {
            for finding in &report.findings {
                writeln!(
                    out,
                    "{}\t{}\t{}",
                    finding.name,
                    finding.taxon.as_deref().unwrap_or("—"),
                    finding.doi.as_deref().unwrap_or("—"),
                )?;
            }
            writeln!(out)?;
            for statement in &report.statements {
                writeln!(out, "{statement}")?;
                writeln!(out)?;
            }
            writeln!(out, "# cite: {}", report.citation)?;
        }
        // `tsv` and `jsonl` are one row per line and carry neither the
        // statements nor the citation. They are for piping into something else,
        // not for curation; `table` and `json` carry both.
        ReportFormat::Tsv => {
            writeln!(out, "name\tsmiles\ttaxon\tdoi\tstatus")?;
            for finding in &report.findings {
                writeln!(
                    out,
                    "{}\t{}\t{}\t{}\tnew",
                    finding.name,
                    finding.smiles,
                    finding.taxon.as_deref().unwrap_or(""),
                    finding.doi.as_deref().unwrap_or(""),
                )?;
            }
        }
        ReportFormat::Json => {
            let value = serde_json::json!({
                "citation": report.citation,
                "findings": report.findings,
                "statements": report.statements,
            });
            serde_json::to_writer_pretty(&mut *out, &value)?;
            writeln!(out)?;
        }
        ReportFormat::Jsonl => {
            for finding in &report.findings {
                serde_json::to_writer(&mut *out, finding)?;
                writeln!(out)?;
            }
        }
    }
    Ok(())
}
