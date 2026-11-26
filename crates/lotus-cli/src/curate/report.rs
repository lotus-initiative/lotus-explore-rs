// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Formatting helpers for `lotus curate` output reports.

use std::io::Write;

use super::{CuratedReport, ReportFormat};

/// The short status name a report carries.
///
/// Stable text, because it is what a person greps for and what a downstream
/// script switches on; the enum variant is a Rust detail.
#[must_use]
pub const fn status_key(status: &lotus_curation::CurationStatus) -> &'static str {
    status.as_key()
}

pub(super) fn write_report<W: Write>(
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
                    sanitize_tsv_cell(&row.input.name),
                    sanitize_tsv_cell(&row.input.smiles),
                    sanitize_tsv_cell(row.input.taxon.as_deref().unwrap_or("")),
                    sanitize_tsv_cell(row.input.doi.as_deref().unwrap_or("")),
                    sanitize_tsv_cell(row.wikidata_qid.as_deref().unwrap_or("")),
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

fn sanitize_tsv_cell(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if matches!(c, '\t' | '\r' | '\n') {
                ' '
            } else {
                c
            }
        })
        .collect()
}
