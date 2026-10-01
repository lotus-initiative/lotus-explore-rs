// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Every relative link in the repository's Markdown must resolve.
//!
//! Links rot quietly: a file is renamed, the link keeps its old target, and
//! nothing fails because nothing reads the rendered page. This checks the
//! target exists instead.
//!
//! Only relative links are checked. An absolute `https://` URL is somebody
//! else's to answer for, and checking it would make `cargo test` depend on the
//! network, which this workspace otherwise does not.

#![allow(unused_crate_dependencies)]

use std::path::{Path, PathBuf};

/// The documents that carry links. Anything not listed is not checked, so adding
/// a file here is a deliberate statement that its links matter.
const DOCUMENTS: &[&str] = &[
    "README.md",
    "CONTRIBUTING.md",
    "docs/ARCHITECTURE.md",
    "docs/cli.md",
    "docs/FRONTENDS.md",
    "crates/lotus-cli/README.md",
    "crates/lotus-model/README.md",
    "crates/lotus-query/README.md",
    "crates/lotus-search/README.md",
    "crates/lotus-curation/README.md",
    "crates/lotus-jsonld/README.md",
];

/// The repository root, from this crate's manifest directory.
fn repo_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|e| format!("the workspace root exists: {e}"))
}

/// Read a file the repository is required to contain.
fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The relative link targets in one line of Markdown.
///
/// Skips images, in-document fragments, and anything carrying a scheme. An
/// `https://` URL is somebody else's to answer for, and checking it would make
/// `cargo test` depend on the network.
fn relative_links(line: &str) -> Vec<String> {
    line.match_indices('[')
        .filter_map(|(start, _)| {
            let after = start + 1;
            // `[` preceded by `!` is an image, not a link.
            if line[..start].ends_with('!') {
                return None;
            }
            let rest = line.get(after..)?;
            let close = rest.find(']')?;
            if rest.get(close + 1..close + 2)? != "(" {
                return None;
            }
            let inner = rest.get(close + 2..)?;
            let end = inner.find(')')?;
            let target = inner.get(..end)?;
            let target = target.split_whitespace().next().unwrap_or_default();
            (!is_external(target)).then(|| target.to_owned())
        })
        .collect()
}

/// `[label]: target`, a reference definition.
fn reference_definition(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix('[')?;
    let (_, target) = rest.split_once("]: ")?;
    let target = target.split_whitespace().next().unwrap_or_default();
    (!target.is_empty() && !is_external(target)).then(|| target.to_owned())
}

/// Whether a link points outside this repository: a fragment, or a URL.
fn is_external(target: &str) -> bool {
    target.is_empty()
        || target.starts_with('#')
        || target.contains("://")
        || target.starts_with("mailto:")
}

/// The `(text, target)` of every relative link in `document`, with line numbers.
fn relative_targets(document: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (index, line) in document.lines().enumerate() {
        for link in relative_links(line)
            .into_iter()
            .chain(reference_definition(line))
        {
            found.push((index + 1, link));
        }
    }
    found
}

/// The anchor target of `link`, with any `#fragment` removed.
fn target_path(link: &str) -> &str {
    link.split(['#', '?']).next().unwrap_or(link)
}

#[test]
fn every_relative_link_resolves() -> Result<(), String> {
    let root = repo_root()?;
    let mut broken: Vec<String> = Vec::new();

    for name in DOCUMENTS {
        let path = root.join(name);
        let document = match read(&path) {
            Ok(text) => text,
            Err(e) => {
                broken.push(format!("{name}: {e}"));
                continue;
            }
        };
        let parent = path.parent().unwrap_or(root.as_path());
        for (line, link) in relative_targets(&document) {
            // A relative link is relative to the document that holds it, not to
            // the repository root.
            let resolved = parent.join(target_path(&link));
            if !resolved.exists() {
                broken.push(format!("{name}:{line}: {link}"));
            }
        }
    }

    assert!(
        broken.is_empty(),
        "these links do not resolve:\n  {}",
        broken.join("\n  ")
    );
    Ok(())
}

#[test]
fn every_documented_task_exists() -> Result<(), String> {
    // A `cargo make <task>` named in the developer docs has to exist, or the
    // instructions cannot be followed. This is the same drift check the old
    // justfile version did, ported to the task runner: a doc that says
    // `cargo make mutants` and a makefile with no `mutants` task is a
    // contributor copying a command that fails.
    //
    // It scans every document `DOCUMENTS` lists, not just the two at the root,
    // because the task names moved into `apps/lotus-explore-rs/README.md` and
    // `docs/FRONTENDS.md` when the task runner replaced the justfile.
    let root = repo_root()?;

    // Every task defined across the root makefile and the files under `make/`.
    let mut tasks: Vec<String> = Vec::new();
    let mut sources = vec![root.join("Makefile.toml")];
    if let Ok(entries) = std::fs::read_dir(root.join("make")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toml") {
                sources.push(path);
            }
        }
    }
    sources.sort();

    for source in &sources {
        let text =
            std::fs::read_to_string(source).map_err(|e| format!("{}: {e}", source.display()))?;
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("[tasks.") else {
                continue;
            };
            let name = rest.split(']').next().unwrap_or_default().trim_matches('"');
            if !name.is_empty() {
                tasks.push(name.to_owned());
            }
        }
    }
    assert!(
        !tasks.is_empty(),
        "no tasks found: the makefiles are missing or the scraper is wrong, and \
         a check that finds nothing passes everything"
    );

    let mut missing = Vec::new();
    for name in DOCUMENTS {
        let path = root.join(name);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            let Some(rest) = line.split("cargo make ").nth(1) else {
                continue;
            };
            let task: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            // `--list-all-steps` and friends are flags, not task names, and they
            // are the one thing the docs tell a contributor to run instead of
            // reading the list. Skipping anything starting with `-` covers the
            // flags without a list that has to be kept up to date.
            if task.is_empty() || task.starts_with('-') {
                continue;
            }
            if !tasks.iter().any(|t| t == &task) {
                missing.push(format!("{name}:{}: cargo make {task}", index + 1));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "these tasks are documented but do not exist:\n  {}",
        missing.join("\n  ")
    );
    Ok(())
}
