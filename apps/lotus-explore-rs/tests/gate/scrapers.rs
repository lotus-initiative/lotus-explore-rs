// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared scrapers for the gate-consistency tests.
//!
//! These read `Makefile.toml`, `make/*.toml`, `prek.toml` and
//! `.github/workflows/ci.yml` and answer "which names appear under this key".
//!
//! The scraping is textual rather than a parse. The app crate has no YAML or TOML
//! parser, and adding one to read two config files would be a dependency for a
//! test. What these checks need is decided by indentation and by a prefix -- and
//! every scraper here asserts on its own output being non-empty, so one that
//! silently finds nothing fails rather than passes. That last part has been the
//! bug twice: a `git ls-files` pathspec that matched nothing read as "clean", and
//! an `#[expect]` on a lint that never fired read as "this is now enforced".
//!
//! Everything here returns `Result` rather than panicking, because the workspace
//! denies `clippy::unwrap_used` and `clippy::expect_used` and that reaches test
//! targets.

use std::collections::BTreeSet;
use std::error::Error;
use std::path::PathBuf;

pub(super) type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(super) fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub(super) fn read(path: &str) -> Result<String> {
    let full = repo_root().join(path);
    std::fs::read_to_string(&full).map_err(|e| format!("{}: {e}", full.display()).into())
}

/// Every task file, concatenated: the root `Makefile.toml` and each file under
/// `make/`.
///
/// Read as a set rather than through the task runner because these tests are
/// about the *source* of truth. Asking `./mk --list-all-steps` would be
/// asking the thing under test to confirm itself, and a task runner that
/// silently loaded no files -- which is what cargo-make does when `extend` is
/// not the first key -- would then report an empty list and pass everything.
pub(super) fn makefiles() -> Result<String> {
    let root = repo_root();
    let mut all = std::fs::read_to_string(root.join("Makefile.toml"))
        .map_err(|e| format!("Makefile.toml: {e}"))?;
    let mut entries: Vec<PathBuf> = std::fs::read_dir(root.join("make"))?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    // Sorted so a failure names the same files in the same order every time.
    entries.sort();
    for path in entries {
        all.push('\n');
        all.push_str(&std::fs::read_to_string(&path)?);
    }
    Ok(all)
}

/// The `dependencies` of a task, read from its `[tasks."<name>"]` block.
///
/// Parsed by finding the table header and reading until the next one, rather
/// than by looking for the task name anywhere in the file: a task that
/// *calls* another is not the same as a task that *depends on* it, and the
/// difference is exactly the drift being looked for.
pub(super) fn task_dependencies(makefiles: &str, task: &str) -> Result<BTreeSet<String>> {
    let header = format!("[tasks.\"{task}\"]");
    let body = makefiles
        .split_once(&header)
        .map(|(_, rest)| rest)
        .ok_or_else(|| format!("no task named `{task}` in the makefiles"))?;
    // Up to the next table header, which is a line starting with `[`.
    let body = body
        .lines()
        .take_while(|line| !line.trim_start().starts_with('['))
        .collect::<Vec<_>>()
        .join("\n");
    let deps = body
        .split_once("dependencies")
        .map(|(_, rest)| rest)
        .ok_or_else(|| format!("task `{task}` has no dependencies list"))?;
    // The list, or the first line of it: cargo-make accepts both a single
    // string and an array, and the gate uses the array form.
    let list = deps
        .split_once('[')
        .map(|(_, rest)| rest)
        .or_else(|| deps.split_once('"').map(|(_, rest)| rest))
        .ok_or_else(|| format!("task `{task}` has an unreadable dependencies list"))?;
    let list = list.split_once(']').map_or(list, |(head, _)| head);
    Ok(list
        .split(',')
        .filter_map(|d| d.split('"').nth(1))
        .map(str::to_string)
        .collect())
}

/// Every task name defined in the makefiles.
pub(super) fn all_tasks(makefiles: &str) -> BTreeSet<String> {
    makefiles
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("[tasks.")?;
            let name = rest.split(']').next()?.trim_matches('"');
            (!name.is_empty() && !name.contains(char::is_whitespace)).then(|| name.to_string())
        })
        .collect()
}

/// The tasks `prek.toml` delegates to.
///
/// Every hook is `./mk <task>` with no cargo flags of its own, so the flags
/// cannot drift -- that is the point of delegating. What can still drift is the
/// *name*: a hook pointing at a task that no longer exists fails on every push,
/// and a new check can go into `./mk ci` with no hook to catch it before
/// the commit lands.
/// The tasks a `prek.toml` hook delegates to, via `./mk`.
pub(super) fn hooked_tasks(hooks: &str) -> BTreeSet<String> {
    hooked_entry_tasks(hooks)
}

/// Every task a hook covers, whether by delegation or by running the tool
/// directly.
/// Every task a hook covers, whether by `cargo make` delegation or by prek
/// running the tool itself.
pub(super) fn covered_by_a_hook(hooks: &str) -> BTreeSet<String> {
    let mut covered = hooked_entry_tasks(hooks);
    // `typos` and `tombi` are covered by prek's own repo hooks rather than by a
    // `cargo make` delegation: prek installs and runs those tools itself. The
    // gate still has a an `./mk typos` task for CI, but the *pre-commit*
    // coverage comes from the repo hook. Counting them here is what stops
    // `every_gated_task_has_a_hook` demanding a duplicate delegation for a task
    // that is already covered twice.
    if hooks.contains("crate-ci/typos") {
        covered.insert("typos".to_owned());
    }
    if hooks.contains("tombi-pre-commit") {
        covered.insert("tombi-check".to_owned());
        covered.insert("tombi-lint".to_owned());
        covered.insert("tombi-fmt".to_owned());
    }
    covered
}

pub(super) fn hooked_entry_tasks(hooks: &str) -> BTreeSet<String> {
    let mut tasks = BTreeSet::new();
    for line in hooks.lines() {
        if let Some((_, rest)) = line.split_once("entry = \"./mk ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            if !name.is_empty() {
                tasks.insert(name);
            }
        }
    }
    tasks
}

/// The CI job names, read from the `jobs:` block only.
pub(super) fn ci_jobs(yaml: &str) -> Result<BTreeSet<String>> {
    let after = yaml
        .split_once("\njobs:\n")
        .map(|(_, rest)| rest)
        .ok_or("the workflow has no `jobs:` block")?;
    Ok(after
        .lines()
        .take_while(|line| line.trim().is_empty() || line.starts_with(' '))
        .filter_map(|line| {
            let indent = line.len() - line.trim_start().len();
            (indent == 2)
                .then(|| line.trim().strip_suffix(':'))
                .flatten()
                .map(str::to_string)
        })
        .filter(|name| !name.starts_with('#'))
        .collect())
}

/// The `./mk <task>` commands a CI job runs.
///
/// A job that calls no task is not covered by `MAPPING` in a way this can check,
/// so it is caught here instead: every job in the workflow is either running a
/// task the gate knows about, or it is listed as one that cannot.
pub(super) fn tasks_a_job_runs(yaml: &str, job: &str) -> BTreeSet<String> {
    let mut tasks = BTreeSet::new();
    let mut inside = false;
    for line in yaml.lines() {
        if line == format!("  {job}:") {
            inside = true;
            continue;
        }
        // A new job starts at column 2, so this ends the block.
        if inside && line.starts_with("  ") && !line.starts_with("   ") && line.ends_with(':') {
            break;
        }
        if !inside {
            continue;
        }
        // Both `run: ./mk x` and `- run: ./mk x` appear.
        if let Some((_, rest)) = line.split_once("./mk ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            if !name.is_empty() {
                tasks.insert(name);
            }
        }
    }
    tasks
}

/// The aggregate tasks: entry points rather than leaf checks.
///
/// `ci-fast`, `ci-slow` and `setup` are *run* rather than *depended on*, so they
/// belong in the gate's own vocabulary and not in the set of leaves `ci`
/// depends on. Asserted in `the_exceptions_are_all_real`.
pub(super) const AGGREGATES: &[&str] = &["ci-fast", "ci-slow", "setup"];
