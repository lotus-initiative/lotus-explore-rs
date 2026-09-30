// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `docs/cli.md` must not drift from `--help`.
//!
//! The help text is the source of truth, because it is generated from the same
//! clap definition the binary uses. A documentation example that no longer
//! parses is worse than a missing one, so every flag the CLI accepts is checked
//! against the documentation, and every documented flag is checked back.

#![allow(unused_crate_dependencies)]
// The panic lints keep library code free of panics on external input. A test
// that fails on a missing file or a bad fixture is reporting, not panicking.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use assert_cmd::Command;

/// The path to the documentation, relative to this crate.
const DOCS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/cli.md");
const README: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/README.md");

/// Split a documented command line into arguments, honouring quotes and
/// dropping a redirection and its target, which are not arguments to `lotus`.
fn shell_words(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut started = false;

    for c in line.chars() {
        match c {
            '"' | '\'' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        args.push(current);
    }

    // Stop at a redirection: the rest is a file name, not arguments.
    let end = args
        .iter()
        .position(|a| a == ">" || a == ">>" || a == "|")
        .unwrap_or(args.len());
    args.truncate(end);
    args
}

fn lotus() -> Command {
    Command::cargo_bin("lotus").expect("the binary is built by cargo test")
}

fn docs() -> String {
    std::fs::read_to_string(DOCS).expect("docs/cli.md exists")
}

/// Every file that documents `lotus` and therefore carries examples that can rot.
///
/// The crate README is here because an example that no longer runs is worse than
/// no example, and a README is the first thing anyone reads. It previously was
/// not covered, and carried a positional argument and a `--doi` flag that `lotus
/// search` has never accepted.
fn documents() -> Vec<(&'static str, String)> {
    [
        ("docs/cli.md", DOCS),
        ("crates/lotus-cli/README.md", README),
    ]
    .into_iter()
    .map(|(name, path)| {
        (
            name,
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path:?}: {e}")),
        )
    })
    .collect()
}

/// Every long flag `lotus search` accepts.
fn search_flags() -> Vec<String> {
    let help = lotus().arg("search").arg("--help").assert().success();
    let help = String::from_utf8(help.get_output().stdout.clone()).expect("UTF-8");

    // A flag line is `  --name <VALUE>` or `  -x, --name <VALUE>`. A line that
    // merely mentions a flag, or lists one of its possible values, is not one.
    help.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let name = match line.strip_prefix("--") {
                Some(rest) => rest,
                None => line.split_once(", --")?.1,
            };
            let name = name.split_whitespace().next()?;
            let name = name.trim_end_matches(['<', ',', ':']);
            (!name.is_empty()).then(|| name.to_string())
        })
        .collect()
}

#[test]
fn every_search_flag_is_documented() {
    let docs = docs();
    let flags = search_flags();
    assert!(flags.len() > 15, "the flag scrape found too few: {flags:?}");

    for flag in flags {
        assert!(
            docs.contains(&format!("--{flag}")),
            "--{flag} is accepted by the CLI but missing from docs/cli.md"
        );
    }
}

#[test]
fn every_documented_flag_actually_exists() {
    let docs = docs();
    let known = search_flags();

    for line in docs.lines() {
        // Only option-table rows and `lotus search` invocations; the install
        // and endpoint sections document other programs' flags.
        let mentions_lotus = line.contains("| `--") || line.contains("lotus search");
        if !mentions_lotus {
            continue;
        }
        for token in line.split_whitespace() {
            let token = token.trim_start_matches('`');
            let Some(documented) = token.strip_prefix("--") else {
                continue;
            };
            let documented =
                documented.trim_end_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-');
            if documented.is_empty() {
                continue;
            }
            assert!(
                known.iter().any(|flag| flag == documented),
                "docs/cli.md mentions --{documented}, which `lotus search` does not accept"
            );
        }
    }
}

#[test]
fn every_subcommand_is_documented() {
    let docs = docs();
    let help = lotus().arg("--help").assert().success();
    let help = String::from_utf8(help.get_output().stdout.clone()).expect("UTF-8");

    for line in help.lines() {
        let Some(name) = line.strip_prefix("  ").map(str::trim) else {
            continue;
        };
        let word = name.split_whitespace().next().unwrap_or_default();
        // `help` is clap's own, and a line starting with a flag is the options
        // list rather than the subcommand list.
        if word.is_empty() || word == "help" || word.starts_with('-') {
            continue;
        }
        if !word.chars().all(|c| c.is_ascii_lowercase()) {
            continue;
        }
        assert!(
            docs.contains(word),
            "the subcommand `{word}` is missing from docs/cli.md"
        );
    }
}

#[test]
fn every_output_format_is_documented() {
    let docs = docs();
    for format in ["table", "tsv", "csv", "json", "jsonl", "jsonld", "query"] {
        assert!(
            docs.contains(format),
            "the output format {format} is missing from docs/cli.md"
        );
    }
}

#[test]
fn the_documented_examples_parse() {
    // An example that no longer runs is worse than no example, so each
    // `lotus …` line in the docs is run with `--explain` appended, which
    // exercises argument parsing without touching the network.
    let mut checked = 0;

    for (name, document) in documents() {
        for line in document.lines() {
            let line = line.trim();
            let Some(rest) = line
                .strip_prefix("lotus ")
                .or_else(|| line.strip_prefix("$ lotus "))
            else {
                continue;
            };
            // Only the `search` subcommand can be checked offline.
            if !rest.starts_with("search ") && rest != "search" {
                continue;
            }
            // A quoted value is one argument, however many spaces it holds.
            let args = shell_words(rest);
            if args.iter().any(|a| a == "--explain") {
                continue;
            }
            let output = lotus()
                .args(&args)
                .arg("--explain")
                .output()
                .expect("the binary runs");
            assert!(
                output.status.success(),
                "{name}: `lotus {rest}` is rejected:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            checked += 1;
        }
    }

    assert!(
        checked >= 10,
        "only {checked} examples were checked: the scraper is probably broken"
    );
}
