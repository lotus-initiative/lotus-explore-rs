// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Top-level CLI argument definitions for `lotus`.

mod search;

pub use search::{LogLevel, Range, SearchArgs};

use crate::curate;
use clap::{Parser, Subcommand};

/// Search LOTUS: chemical compounds reported in organisms, from Wikidata.
#[derive(Debug, Parser)]
#[command(
    name = "lotus",
    version,
    about = "Search the LOTUS natural-products knowledge graph over SPARQL",
    long_about = "Search the LOTUS knowledge graph of chemical compounds, the organisms \
                  they occur in, and the references that report them. The data is the \
                  Wikidata projection of the LOTUS database, queried over SPARQL.\n\n\
                  Every filter the web explorer offers is available here, and the \
                  output formats are the same, so a result set can be moved between \
                  the two without changing anything but the command line."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Find compounds, taxa and the references that report them.
    Search(SearchArgs),
    /// Check what Wikidata already has for a set of compounds, and emit the
    /// edits needed to complete it.
    Curate(curate::CurateArgs),
    /// Print a shell completion script to stdout.
    Completions(CompletionsArgs),
    /// Write the manual page to stdout.
    Man,
    /// Export the full LOTUS compound-taxon-reference graph as CSV files.
    Export(ExportArgs),
    /// Build `PubChem`-format trees (biological, chemical, `NPClassifier`) as JSON.
    ExportPubchemTree(ExportPubchemTreeArgs),
    /// Archive LOTUS SMILES from Wikidata as a versioned tar.gz with manifest and checksums.
    Archive(ArchiveArgs),
}

#[derive(Debug, clap::Args)]
pub struct CompletionsArgs {
    /// The shell to generate for.
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,
}

/// Where the export files are written, and the year and timestamp they carry.
#[derive(Debug, clap::Args)]
pub struct ExportArgs {
    /// Directory to write `frozen.csv` and `frozen_metadata.csv` into.
    /// Defaults to the current working directory.
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<std::path::PathBuf>,

    /// Year recorded in the export metadata. Defaults to this year. It does
    /// not filter: the export is the whole graph.
    #[arg(long)]
    pub year: Option<u16>,

    /// ISO-8601 timestamp for the export metadata. Defaults to now.
    #[arg(long)]
    pub now: Option<String>,
}

/// Where the `PubChem` tree `JSON` files are written.
#[derive(Debug, clap::Args)]
pub struct ExportPubchemTreeArgs {
    /// Directory to write the six tree JSON files into.
    /// Defaults to the current working directory.
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<std::path::PathBuf>,
}

/// Archive LOTUS SMILES from Wikidata as a tar.gz with manifest and checksums.
///
/// By default the archive is written to disk without contacting Zenodo. Pass
/// `--publish --dry-run` to validate Zenodo metadata without uploading, or
/// `--publish` with `ZENODO_TOKEN` to publish.
#[derive(Debug, clap::Args)]
pub struct ArchiveArgs {
    /// Directory to write the archive files into. Defaults to the current
    /// working directory.
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<std::path::PathBuf>,

    /// Validate the Zenodo metadata and upload plan without contacting Zenodo
    /// (requires `--publish`).
    #[arg(long, requires = "publish")]
    pub dry_run: bool,

    /// Publish the completed archive to Zenodo using `ZENODO_TOKEN`.
    /// Requires either `ZENODO_ROOT_DEPOSITION_ID` (new version) or nothing
    /// (initial record). CI must not set this flag.
    #[arg(long, default_value_t = false)]
    pub publish: bool,

    /// Creator name required by Zenodo metadata.
    #[arg(long, default_value = "The LOTUS Initiative")]
    pub creator: String,

    /// ISO-8601 timestamp for the archive manifest. Defaults to now.
    #[arg(long)]
    pub now: Option<String>,
}
