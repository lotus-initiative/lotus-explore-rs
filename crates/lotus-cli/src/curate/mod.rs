// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `lotus curate` — check a TSV of findings against Wikidata.
//!
//! Nothing here writes to Wikidata. The output is a set of `QuickStatements` that
//! a curator reviews and submits, and there is no flag that submits for them:
//! running the command twice cannot double-submit anything, because running it
//! submits nothing.

mod report;
mod runner;

use std::io::{self, Read as _, Write};
use std::process::ExitCode;

use clap::ValueEnum;

#[cfg(test)]
use report::status_key;
use report::write_report;
use runner::{curate_findings, should_warn_about_unchecked, unchecked_count};

#[cfg(test)]
use runner::{collect_report_statements, convertible_smiles, curate_row, not_checked_row};

/// What `curate` does with a set of rows.
#[derive(Debug, clap::Args)]
pub struct CurateArgs {
    /// TSV or CSV table with `name` and `smiles` columns, and optionally `taxon`
    /// and `doi`. Reads stdin when omitted or `-`.
    #[arg(value_name = "TABLE")]
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
    /// Every non-revised row is reported as `not_checked`, not as `new` (rows
    /// with `status = revised` report `existing_updates` and suppress `CREATE`
    /// statements). A row that was never looked up has not been shown to be
    /// absent, and the difference between "I did not look" and "it is not
    /// there" is the difference between an honest report and a duplicate
    /// submission.
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
    let unchecked = unchecked_count(&report);
    if should_warn_about_unchecked(unchecked, args.quiet) {
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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
