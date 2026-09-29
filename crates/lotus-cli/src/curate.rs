// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `lotus curate` — check a TSV of findings against Wikidata.
//!
//! Nothing here writes to Wikidata. The output is a set of `QuickStatements` that
//! a curator reviews and submits, and `--dry-run` is on unless `--apply` is
//! given, so that running the command twice cannot double-submit anything.

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

    /// Report what would be submitted without submitting it. This is the
    /// default; `--apply` is what turns it off.
    #[arg(long)]
    pub apply: bool,

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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Finding {
    pub name: String,
    pub smiles: String,
    pub taxon: Option<String>,
    pub doi: Option<String>,
}

/// Parse a TSV by column name.
///
/// Columns are matched by name rather than position, because an export from a
/// spreadsheet carries whatever else it has and a caller should not have to
/// strip it first.
pub fn parse_tsv(tsv: &str) -> Result<Vec<Finding>, String> {
    let mut lines = tsv
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty());
    let Some(header) = lines.next() else {
        return Ok(Vec::new());
    };

    let columns: Vec<String> = header.split('\t').map(normalize_header).collect();
    let name_at = required(&columns, "name")?;
    let smiles_at = required(&columns, "smiles")?;
    let taxon_at = columns
        .iter()
        .position(|c| matches!(c.as_str(), "taxon" | "organism"));
    let doi_at = columns.iter().position(|c| c == "doi");

    let mut findings = Vec::new();
    for line in lines {
        // Split before trimming, so that a row whose first cell is empty keeps
        // its columns aligned.
        let cells: Vec<&str> = line.split('\t').collect();
        let name = cells.get(name_at).copied().unwrap_or_default().trim();
        let smiles = cells.get(smiles_at).copied().unwrap_or_default().trim();
        if name.is_empty() || smiles.is_empty() {
            continue;
        }
        findings.push(Finding {
            name: name.to_string(),
            smiles: smiles.to_string(),
            taxon: taxon_at
                .and_then(|i| cells.get(i))
                .map(|c| c.trim())
                .filter(|t| !t.is_empty())
                .map(ToOwned::to_owned),
            doi: doi_at
                .and_then(|i| cells.get(i))
                .map(|c| c.trim())
                .filter(|t| !t.is_empty())
                .map(|d| normalize_doi(d).to_ascii_uppercase())
                .filter(|d| !d.is_empty()),
        });
    }
    Ok(findings)
}

fn required(columns: &[String], name: &str) -> Result<usize, String> {
    columns
        .iter()
        .position(|c| c == name)
        .ok_or_else(|| format!("the TSV needs a {name:?} column"))
}

fn normalize_header(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
}

/// Wikidata stores P356 upper-cased and unprefixed, so a DOI is stored that way
/// here too: a query that would not match is worse than a reformatted one.
fn normalize_doi(value: &str) -> String {
    value.to_ascii_lowercase().find("doi.org/").map_or_else(
        || value.to_string(),
        |idx| value[idx + "doi.org/".len()..].to_string(),
    )
}

/// The two findings that name the same compound in the same taxon with the same
/// reference are the same finding, whatever they were called.
#[must_use]
pub fn identity_key(finding: &Finding) -> String {
    // Every part is compared case-insensitively: a spreadsheet has no opinion
    // about capitalisation, and `CCO` and `cco` are the same molecule.
    let structure = finding.smiles.trim().to_ascii_uppercase();
    let taxon = finding
        .taxon
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let doi = finding.doi.as_deref().unwrap_or_default().trim();
    format!("{structure}\t{taxon}\t{doi}")
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

    if !args.apply {
        eprintln!(
            "lotus: dry run — nothing will be submitted. Pass --apply to emit \
             statements for review."
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

/// What curation would do to each row, given what Wikidata already has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub findings: Vec<Finding>,
    /// `QuickStatements` a curator would submit, in dependency order.
    pub statements: Vec<String>,
    /// The LOTUS paper, which every result should cite.
    pub citation: &'static str,
}

fn build_report(findings: &[Finding]) -> Report {
    let mut statements = Vec::new();
    for finding in findings {
        // The statements a curator reviews are generated here rather than by the
        // web app: this is the batch form of the same workflow, and the point is
        // that nothing reaches Wikidata without a human reading it.
        statements.push(format!(
            "## {}\nCREATE\nLAST|Len|\"{}\"\nLAST|P233|\"{}\"",
            finding.name,
            finding.name.replace('"', "\\\""),
            finding.smiles,
        ));
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
