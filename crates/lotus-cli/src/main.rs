// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `lotus` — search the LOTUS knowledge graph from a terminal.
//!
//! A thin layer over `lotus-model` (filters), `lotus-search` (the search
//! sequence) and `lotus-jsonld` (JSON-LD). Every semantic decision lives in
//! those crates, so the CLI and the web app cannot drift apart.

// `cargo test` builds this binary twice: once as the shipped artifact, and once
// with `--test` so the integration tests can find it. In the second build the
// dev-dependencies are linked in and the binary itself uses none of them, so
// `unused_crate_dependencies` cannot be satisfied under `--all-targets`. The
// real dependencies are checked in `src/` by not being listed unless used.
#![allow(
    unused_crate_dependencies,
    reason = "the `--test` build links the dev-dependencies without the binary using them"
)]
// This is a binary: there is nothing outside the crate for `pub` to reach, so
// `unreachable_pub` reports every module and item that cross a module boundary.
// The items are `pub` only so that `main` can name them.
#![allow(
    unreachable_pub,
    reason = "a binary has no external consumer, so every `pub` item crosses a module boundary for nothing"
)]

use std::io::{self, Write};
use std::process::ExitCode;

use clap::Parser;

mod archive_cmd;
mod args;
mod clock;
mod criteria;
mod curate;
mod export_cmd;
mod output;
mod search_cmd;
mod zenodo;

use args::{Cli, Command};

#[cfg(test)]
use archive_cmd::{
    archive, archive_filename, archive_upload_paths, dry_run_zenodo, parse_deposition_id,
    publish_to_zenodo, validate_source_code_url, write_archive_files, zenodo_metadata,
};
#[cfg(test)]
use args::{ArchiveArgs, Range, SearchArgs};
pub(crate) use clock::now_iso_utc;
#[cfg(test)]
use clock::{current_year, iso_utc, parse_iso_utc};
#[cfg(test)]
use criteria::{apply_ranges, criteria_from_args, flags_ask_for_formula};
#[cfg(test)]
use export_cmd::ExportProgress;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("lotus: could not start the async runtime: {err}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run(cli)) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("lotus: {err:#}");
            let _ = io::stderr().flush();
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    match cli.command {
        Command::Search(args) => search_cmd::search(args).await,
        Command::Curate(args) => curate::run(&args).await,
        Command::Completions(args) => {
            let mut command = <Cli as clap::CommandFactory>::command();
            let name = command.get_name().to_string();
            clap_complete::generate(args.shell, &mut command, name, &mut io::stdout().lock());
            Ok(ExitCode::SUCCESS)
        }
        Command::Man => {
            clap_mangen::Man::new(<Cli as clap::CommandFactory>::command())
                .render(&mut io::stdout().lock())?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Export(args) => export_cmd::export(args).await,
        Command::ExportPubchemTree(args) => export_cmd::export_pubchem_tree(args).await,
        Command::Archive(args) => Box::pin(archive_cmd::archive(args)).await,
    }
}

#[cfg(test)]
#[path = "main/range_tests.rs"]
mod range_tests;

#[cfg(test)]
#[path = "main/clock_tests.rs"]
mod clock_tests;

#[cfg(test)]
#[path = "main/nomenclature_flag_tests.rs"]
mod nomenclature_flag_tests;

#[cfg(test)]
#[path = "main/archive_tests.rs"]
mod archive_tests;

#[cfg(test)]
#[path = "main/export_progress_tests.rs"]
mod export_progress_tests;
