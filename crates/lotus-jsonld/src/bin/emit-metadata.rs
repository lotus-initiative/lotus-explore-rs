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

#![allow(unused_crate_dependencies)]
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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.iter().any(|a| a == "--check");
    let root = args
        .iter()
        .find_map(|a| a.strip_prefix("--root="))
        .map_or_else(|| ".".to_string(), std::borrow::ToOwned::to_owned);

    let files = [
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
    ];

    let mut stale = Vec::new();
    for (name, contents) in &files {
        let path = std::path::Path::new(&root).join(name);
        if check {
            match std::fs::read_to_string(&path) {
                Ok(current) if current == *contents => {}
                Ok(_) | Err(_) => stale.push((*name).to_string()),
            }
            continue;
        }
        if let Err(err) = std::fs::write(&path, contents) {
            eprintln!("could not write {}: {err}", path.display());
            return ExitCode::FAILURE;
        }
        println!("wrote {}", path.display());
    }

    if check && !stale.is_empty() {
        eprintln!(
            "out of date, run `cargo run -p lotus-jsonld --bin emit-metadata`: {}",
            stale.join(", ")
        );
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
