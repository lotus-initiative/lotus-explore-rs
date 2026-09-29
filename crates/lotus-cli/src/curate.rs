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

// The escaper lives in the curation crate because the web client builds the same
// statements, and two escapers is one duplicated bug waiting to happen.
use lotus_curation::escape_quickstatements;

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

    /// Suppress the reminders on stderr: that nothing has been submitted, and how
    /// many rows were not looked up.
    ///
    /// Nothing is ever submitted. This only silences the reminders, for when the
    /// output is being piped somewhere that reads stderr as noise. Both are
    /// status, not error, so both go. There is
    /// deliberately no flag that submits: writing to Wikidata is a decision
    /// with a person attached to it, and a batch of a few hundred statements
    /// arriving from a shell script at 3am is not one.
    #[arg(long)]
    pub quiet: bool,

    /// Log to stderr: error, warn, info, debug.
    #[arg(long, value_enum, default_value_t = LogLevel::Error)]
    pub log: LogLevel,

    /// Do not contact Wikidata, and report every row as not looked up.
    ///
    /// Wikidata needs a structure converted to an `InChIKey` before it can be
    /// matched at all, and that conversion is a network call, so a run makes two
    /// or more per row. This skips all of it.
    ///
    /// Every row is reported as `not_checked`, not as `new`. A row that was never
    /// looked up has not been shown to be absent, and the difference between
    /// "I did not look" and "it is not there" is the difference between an honest
    /// report and a duplicate submission.
    #[arg(long)]
    pub offline: bool,
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

pub async fn run(args: &CurateArgs) -> anyhow::Result<ExitCode> {
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

    let report = curate_findings(&unique, args.offline).await?;

    // Worth saying out loud, because a bundle that reads like a finished curation
    // is the one output of this command that can do damage. Under `--offline`
    // this is every row.
    let unchecked = report
        .rows
        .iter()
        .filter(|row| row.status == lotus_curation::CurationStatus::NotChecked)
        .count();
    if unchecked > 0 && !args.quiet {
        eprintln!("lotus: {unchecked} row(s) were not looked up");
    }

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
#[derive(Debug, Clone, PartialEq)]
pub struct CuratedReport {
    /// One entry per input row, in the order they were read.
    pub rows: Vec<lotus_curation::CurationResultRow>,
    /// `QuickStatements` a curator would submit, deduplicated and in order.
    pub statements: Vec<String>,
    /// The LOTUS paper, which every result should cite.
    pub citation: &'static str,
}

/// The properties a LOTUS row is written against, from the Wikidata projection.
///
/// `P233` is the canonical SMILES and `P2017` the isomeric one. Both are emitted
/// because the explorer queries both: a row with only `P233` matches a
/// structure search by canonical form and not by the form the chemist submitted.
const PROPERTY_CANONICAL_SMILES: &str = "P233";
const PROPERTY_ISOMERIC_SMILES: &str = "P2017";
const PROPERTY_NAME: &str = "Len";

/// Curate one row against Wikidata.
///
/// The structure arrives already converted, because the `InChIKey` is the only
/// thing Wikidata can be asked about: a `SMILES` is not an identity, and
/// matching on one would report a compound as new whenever the row happened to
/// write it in a different valid order.
async fn curate_row<H: lotus_search::Http>(
    http: &H,
    finding: &Finding,
    converted: &lotus_curation::ConvertedStructure,
) -> lotus_curation::CurationResultRow {
    match lotus_curation::look_up(http, finding, converted).await {
        Ok(lookup) => {
            let mut result = lotus_curation::to_result_row(finding, converted, &lookup);
            // The `InChI` the conversion produced is worth keeping even when
            // Wikidata has none of its own.
            if result.inchi.is_none() {
                result.inchi.clone_from(&converted.inchi);
            }
            result
        }
        Err(err) => {
            let mut result = lotus_curation::to_result_row(
                finding,
                converted,
                &lotus_curation::WikidataLookup::default(),
            );
            // A lookup that failed is not a compound that is absent, so the row is
            // an error and not a new item.
            result.status = lotus_curation::CurationStatus::Error;
            result.note = format!("Wikidata could not be reached: {err}");
            result
        }
    }
}

/// Report a row that was never looked up.
///
/// The statements are still generated, because a curator offline still wants
/// something to read, but the status says the row is unchecked so the bundle is
/// not mistaken for a finished curation.
fn not_checked_row(finding: &Finding) -> lotus_curation::CurationResultRow {
    let mut statements = vec![format!(
        "## {name}\nCREATE\nLAST|{PROPERTY_NAME}|\"{name}\"\n\
         LAST|{PROPERTY_CANONICAL_SMILES}|\"{smiles}\"\nLAST|{PROPERTY_ISOMERIC_SMILES}|\"{smiles}\"",
        name = escape_quickstatements(finding.name.trim()),
        smiles = escape_quickstatements(finding.smiles.trim()),
    )];
    if let Some(taxon) = finding
        .taxon
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        statements.push(format!(
            "\n## {name} — occurrence\nCREATE\nLAST|P703|\"{taxon}\"",
            name = escape_quickstatements(finding.name.trim()),
            taxon = escape_quickstatements(taxon),
        ));
    }
    if let Some(doi) = finding
        .doi
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        statements.push(format!(
            "\n## {name} — reference\nCREATE\nLAST|P248|\"{doi}\"",
            name = escape_quickstatements(finding.name.trim()),
            doi = escape_quickstatements(doi),
        ));
    }

    lotus_curation::CurationResultRow {
        input: finding.clone(),
        canonical_smiles: None,
        inchikey: None,
        inchi: None,
        formula: None,
        exact_mass: None,
        mass_warning: None,
        wikidata_qid: None,
        status: lotus_curation::CurationStatus::NotChecked,
        note: "not looked up: run without --offline to check this against Wikidata".into(),
        dependency_blocks: Vec::new(),
        quickstatements: statements,
    }
}

/// Curate every row, in order.
///
/// The structures are converted for the whole file in one batch, because
/// converting them a row at a time is three requests per row and the public
/// service that does the converting rate-limits that. The Wikidata lookups stay
/// per row: they are a different service, and a `VALUES` query over a whole file
/// would be one enormous answer to hold in memory and lose everything if one row
/// in it were wrong.
async fn curate_findings(findings: &[Finding], offline: bool) -> anyhow::Result<CuratedReport> {
    let mut rows = Vec::with_capacity(findings.len());

    if offline {
        rows.extend(findings.iter().map(not_checked_row));
    } else {
        let http = lotus_search::reqwest_client::ReqwestClient::new()?;

        let smiles: Vec<&str> = findings
            .iter()
            .map(|finding| finding.smiles.trim())
            .filter(|smiles| !smiles.is_empty())
            .collect();
        let converted = lotus_curation::convert_structures(&http, &smiles).await?;

        let mut by_smiles: std::collections::HashMap<&str, &lotus_curation::ConvertedStructure> =
            std::collections::HashMap::new();
        for (smiles, structure) in smiles.iter().zip(&converted) {
            by_smiles.insert(smiles, structure);
        }

        for finding in findings {
            let key = finding.smiles.trim();
            match by_smiles.get(key) {
                Some(structure) => rows.push(curate_row(&http, finding, structure).await),
                // No entry means the file had no structure to convert, which
                // `parse_tsv` should have caught; a row that gets here has no
                // structure at all and is reported rather than skipped.
                None => rows.push(
                    curate_row(
                        &http,
                        finding,
                        &lotus_curation::ConvertedStructure::default(),
                    )
                    .await,
                ),
            }
        }
    }

    let mut statements = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for row in &rows {
        for block in &row.dependency_blocks {
            if seen.insert(block.clone()) {
                statements.push(block.clone());
            }
        }
        for block in &row.quickstatements {
            if seen.insert(block.clone()) {
                statements.push(block.clone());
            }
        }
    }

    Ok(CuratedReport {
        rows,
        statements,
        citation: "https://doi.org/10.7554/eLife.70780",
    })
}

/// The short status name a report carries.
///
/// Stable text, because it is what a person greps for and what a downstream
/// script switches on; the enum variant is a Rust detail.
#[must_use]
pub const fn status_key(status: &lotus_curation::CurationStatus) -> &'static str {
    use lotus_curation::CurationStatus;
    match status {
        CurationStatus::ExistingComplete => "existing_complete",
        CurationStatus::ExistingNeedsUpdates => "existing_updates",
        CurationStatus::NewCompound => "new_compound",
        CurationStatus::PendingDependencies => "pending_dependencies",
        CurationStatus::NotChecked => "not_checked",
        CurationStatus::Error => "error",
    }
}

fn write_report<W: Write>(
    out: &mut W,
    report: &CuratedReport,
    format: ReportFormat,
) -> anyhow::Result<()> {
    match format {
        ReportFormat::Table => {
            for row in &report.rows {
                writeln!(
                    out,
                    "{}\t{}\t{}\t{}\t{}",
                    row.input.name,
                    row.input.taxon.as_deref().unwrap_or("—"),
                    row.input.doi.as_deref().unwrap_or("—"),
                    row.wikidata_qid.as_deref().unwrap_or("—"),
                    status_key(&row.status),
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
            writeln!(out, "name\tsmiles\ttaxon\tdoi\twikidata_qid\tstatus")?;
            for row in &report.rows {
                writeln!(
                    out,
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    row.input.name,
                    row.input.smiles,
                    row.input.taxon.as_deref().unwrap_or(""),
                    row.input.doi.as_deref().unwrap_or(""),
                    row.wikidata_qid.as_deref().unwrap_or(""),
                    status_key(&row.status),
                )?;
            }
        }
        ReportFormat::Json => {
            let value = serde_json::json!({
                "citation": report.citation,
                "rows": report.rows,
                "statements": report.statements,
            });
            serde_json::to_writer_pretty(&mut *out, &value)?;
            writeln!(out)?;
        }
        ReportFormat::Jsonl => {
            for row in &report.rows {
                serde_json::to_writer(&mut *out, row)?;
                writeln!(out)?;
            }
        }
    }
    Ok(())
}
