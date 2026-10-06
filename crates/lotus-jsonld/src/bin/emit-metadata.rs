// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Write `codemeta.json` and `CITATION.cff` from one description of the
//! software.
//!
//! Generated rather than hand-maintained. Two files describing the same project
//! will disagree, and the one nobody remembers to update is the one a citation
//! gets taken from. `--check` is the useful mode for CI: it reports whether the
//! committed files are current without writing them, so a version bump that
//! forgot to regenerate them fails the build instead of publishing a stale
//! citation.

#![allow(
    unused_crate_dependencies,
    reason = "a binary links its dependencies to run, not to call them"
)]
// A binary exists to be run, not to be called, so a panic here is a bug report
// rather than a way to handle bad input from a caller.
#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::process::ExitCode;

use lotus_jsonld::{SOFTWARE, citation_cff, codemeta};

/// What one run of the generator should do.
#[derive(Debug, PartialEq, Eq)]
struct Options {
    /// Report which files are out of date instead of writing them.
    check: bool,
    /// Directory the generated files live in.
    root: String,
}

impl Options {
    /// Read the options out of the arguments after the program name.
    fn parse(args: &[String]) -> Self {
        Self {
            check: args.iter().any(|arg| arg == "--check"),
            root: args
                .iter()
                .find_map(|arg| arg.strip_prefix("--root="))
                .map_or_else(|| ".".to_owned(), ToOwned::to_owned),
        }
    }
}

/// The files every run produces, in a fixed order.
fn files() -> [(&'static str, String); 2] {
    [
        // Pretty-printed: a committed metadata file is read and diffed by
        // people, and a single line of it is not.
        (
            "codemeta.json",
            format!(
                "{}\n",
                serde_json::to_string_pretty(&codemeta(&SOFTWARE)).unwrap_or_default()
            ),
        ),
        ("CITATION.cff", citation_cff(&SOFTWARE)),
    ]
}

/// Which of `files` are not what is on disk.
///
/// A file that cannot be read counts as stale rather than being skipped: a
/// missing `codemeta.json` is exactly what `--check` is meant to catch, and
/// reporting nothing for it would make the check pass on an empty directory.
fn stale_files(root: &std::path::Path, files: &[(&str, String)]) -> Vec<String> {
    files
        .iter()
        .filter_map(|(name, contents)| {
            let path = root.join(name);
            match std::fs::read_to_string(&path) {
                Ok(current) if current == *contents => None,
                Ok(_) | Err(_) => Some((*name).to_string()),
            }
        })
        .collect()
}

/// Write every file, reporting the first that could not be written.
fn write_files(root: &std::path::Path, files: &[(&str, String)]) -> std::io::Result<()> {
    for (name, contents) in files {
        let path = root.join(name);
        std::fs::write(&path, contents)
            .map_err(|err| std::io::Error::new(err.kind(), format!("{}: {err}", path.display())))?;
    }
    Ok(())
}

fn run(options: &Options) -> ExitCode {
    let root = std::path::Path::new(&options.root);
    let files = files();

    if options.check {
        let stale = stale_files(root, &files);
        if !stale.is_empty() {
            eprintln!(
                "out of date, run `cargo run -p lotus-jsonld --bin emit-metadata`: {}",
                stale.join(", ")
            );
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }

    if let Err(err) = write_files(root, &files) {
        eprintln!("could not write metadata: {err}");
        return ExitCode::FAILURE;
    }
    for (name, _) in &files {
        println!("wrote {}", root.join(name).display());
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&Options::parse(&args))
}

#[cfg(test)]
#[path = "emit-metadata/tests.rs"]
mod tests;
