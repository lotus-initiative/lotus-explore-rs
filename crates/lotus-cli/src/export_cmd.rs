// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Handlers for `lotus export` and `lotus export-pubchem-tree`.

use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

use lotus_export::pubchem_tree::{PUBCHEM_TREE_QUERY_COUNT, PubchemTreeOptions, run_pubchem_tree};

use crate::args::{ExportArgs, ExportPubchemTreeArgs};
use crate::clock::{current_year, now_iso_utc};

/// Character width of the visual progress bar (`[====>---]`).
const PROGRESS_BAR_WIDTH: usize = 40;

/// Transient single-line `stderr` progress bar for multi-query exports.
pub struct ExportProgress<W> {
    pub writer: W,
    is_terminal: bool,
    completed: u64,
    total: u64,
}

impl<W: Write> ExportProgress<W> {
    pub const fn new(writer: W, is_terminal: bool, total: u64) -> Self {
        Self {
            writer,
            is_terminal,
            completed: 0,
            total,
        }
    }

    pub fn step(&mut self, table: &'static str) {
        if self.is_terminal && self.total > 0 {
            let completed = self.completed.min(self.total);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "completed <= total and PROGRESS_BAR_WIDTH is 40, so the quotient fits in usize"
            )]
            let filled = ((completed * PROGRESS_BAR_WIDTH as u64) / self.total) as usize;
            let rest = PROGRESS_BAR_WIDTH.saturating_sub(filled);
            let _ = write!(
                self.writer,
                "\r\x1b[2K[\x1b[36m{:=<filled$}>\x1b[34m{:->rest$}\x1b[0m] {}/{} Fetching {table}",
                "", "", completed, self.total
            );
            let _ = self.writer.flush();
        }
        self.completed = self.completed.saturating_add(1);
    }

    pub fn finish(&mut self) {
        if self.is_terminal {
            let _ = write!(self.writer, "\r\x1b[2K");
            let _ = self.writer.flush();
        }
    }
}

/// Export the full LOTUS compound–taxon–reference graph as `frozen.csv` and
/// `frozen_metadata.csv`, written to a directory the caller picks.
///
/// No filters: this is the bulk exporter, the same source as
/// `lotus_exporter.py`, and it writes everything it can fetch. The year and
/// timestamp the files carry come from the flags (or the clock), and this
/// function is where the native file I/O lives — `run_lotus_exporter` itself
/// performs none, so it stays wasm-clean.
pub async fn export(args: ExportArgs) -> anyhow::Result<ExitCode> {
    let year = args.year.unwrap_or_else(current_year);
    let now = match args.now {
        Some(raw) => {
            crate::clock::parse_iso_utc(&raw)?;
            raw
        }
        None => now_iso_utc(),
    };

    let http = lotus_search::reqwest_client::ReqwestClient::new()?;
    let endpoint = lotus_search::Endpoint::new(lotus_search::Service::Qlever);

    let stderr = io::stderr();
    let is_terminal = stderr.is_terminal();
    let mut progress = ExportProgress::new(
        stderr,
        is_terminal,
        lotus_export::pipeline::EXPORT_QUERY_COUNT,
    );
    let result = lotus_export::pipeline::run_lotus_exporter(
        &http,
        &lotus_export::pipeline::PipelineOptions {
            endpoint,
            now: &now,
            year,
        },
        |table| progress.step(table),
    )
    .await;
    progress.finish();
    let (frozen_csv, metadata_csv, summary) = result?;

    let output_dir = args
        .output
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(output_dir)?;

    let frozen_path = output_dir.join("frozen.csv");
    let metadata_path = output_dir.join("frozen_metadata.csv");
    std::fs::write(&frozen_path, frozen_csv)?;
    std::fs::write(&metadata_path, metadata_csv)?;

    eprintln!(
        "lotus: wrote {} rows ({} compounds, {} taxa, {} references) to {} and {}",
        summary.rows_written,
        summary.compounds,
        summary.taxa,
        summary.references,
        frozen_path.display(),
        metadata_path.display(),
    );
    Ok(ExitCode::SUCCESS)
}

/// Build `PubChem`-format trees (biological, chemical, `NPClassifier`) as `JSON`,
/// writing six files to an output directory. No network transport beyond the
/// SPARQL endpoint and the `NPClassifier` cache fetch.
pub async fn export_pubchem_tree(args: ExportPubchemTreeArgs) -> anyhow::Result<ExitCode> {
    let now = now_iso_utc();

    let http = lotus_search::reqwest_client::ReqwestClient::new()?;
    let endpoint = lotus_search::Endpoint::new(lotus_search::Service::Qlever);

    let stderr = io::stderr();
    let is_terminal = stderr.is_terminal();
    let mut progress = ExportProgress::new(stderr, is_terminal, PUBCHEM_TREE_QUERY_COUNT);
    let result = run_pubchem_tree(
        &http,
        &PubchemTreeOptions {
            endpoint,
            now: &now,
        },
        |table| progress.step(table),
    )
    .await;
    progress.finish();
    let outputs = result?;

    let output_dir = args
        .output
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(output_dir)?;

    let write = |name: &str, content: &str| -> anyhow::Result<()> {
        let path = output_dir.join(name);
        std::fs::write(&path, content)?;
        eprintln!("lotus: wrote {} bytes to {}", content.len(), path.display());
        Ok(())
    };

    write("lotus_biological_tree.json", &outputs.biological_full)?;
    write("lotus_chemical_tree.json", &outputs.chemical_full)?;
    write("lotus_npclassifier_tree.json", &outputs.npclassifier_full)?;
    write(
        "lotus_biological_tree_pubchem.json",
        &outputs.biological_pubchem,
    )?;
    write(
        "lotus_chemical_tree_pubchem.json",
        &outputs.chemical_pubchem,
    )?;
    write(
        "lotus_npclassifier_tree_pubchem.json",
        &outputs.npclassifier_pubchem,
    )?;

    Ok(ExitCode::SUCCESS)
}
