// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The local gate, the git hooks and the CI gate must be the same gate.
//!
//! `./mk ci`, `prek.toml` and `.github/workflows/ci.yml` are three
//! hand-maintained lists of the same checks, and hand-maintained lists drift.
//! They had: the local gate ran checks CI did not, CI ran a job the local gate
//! did not, four gated tasks had no hook at all, and the desktop job had no
//! local equivalent -- which is the job that would have caught the unstyled
//! window, the no-op download and the missing structure toolkit.
//!
//! `MAPPING` below is the correspondence, written out once. These tests hold it
//! to three things: it covers every CI job, every task it names really exists,
//! and `./mk ci` runs exactly the part of it meant to be local. Adding a
//! check to one file and forgetting the others now fails the build.
//!
//! The scraping is textual rather than a parse. The app crate has no YAML or
//! TOML parser, and adding one to read two config files would be a dependency
//! for a test. What these checks need is "which names appear under this key",
//! which is decided by indentation and by a prefix -- and every scraper below
//! asserts on its own output being non-empty, so one that silently finds nothing
//! fails rather than passes.
//!
//! The tests return `Result` rather than panicking, because the workspace denies
//! `clippy::unwrap_used` and `clippy::expect_used` and that reaches test targets.

#![allow(unused_crate_dependencies)] // links the crate's deps without using them

use std::collections::BTreeSet;
use std::error::Error;
use std::path::PathBuf;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> Result<String> {
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
fn makefiles() -> Result<String> {
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
fn task_dependencies(makefiles: &str, task: &str) -> Result<BTreeSet<String>> {
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
fn all_tasks(makefiles: &str) -> BTreeSet<String> {
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
fn hooked_tasks(hooks: &str) -> BTreeSet<String> {
    hooked_entry_tasks(hooks)
}

/// Every task a hook covers, whether by delegation or by running the tool
/// directly.
/// Every task a hook covers, whether by `cargo make` delegation or by prek
/// running the tool itself.
fn covered_by_a_hook(hooks: &str) -> BTreeSet<String> {
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

fn hooked_entry_tasks(hooks: &str) -> BTreeSet<String> {
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
fn ci_jobs(yaml: &str) -> Result<BTreeSet<String>> {
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
fn tasks_a_job_runs(yaml: &str, job: &str) -> BTreeSet<String> {
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
const AGGREGATES: &[&str] = &["ci-fast", "ci-slow", "setup"];

/// One CI job, the local tasks that cover it, and whether it gates.
struct Job {
    name: &'static str,
    /// Tasks whose combined commands are what this job runs.
    tasks: &'static [&'static str],
    /// `false` for the jobs `./mk ci` deliberately leaves to
    /// `./mk ci-slow`, or that CI runs and the local gate cannot.
    in_local_gate: bool,
    /// Why, for a job that is not in the local gate. Asserted non-empty.
    note: &'static str,
}

/// The correspondence, in CI's order.
const MAPPING: &[Job] = &[
    Job {
        name: "ci",
        tasks: &["ci"],
        in_local_gate: true,
        note: "",
    },
    Job {
        name: "coverage",
        tasks: &["cov"],
        in_local_gate: false,
        note: "rebuilds the whole graph under instrumentation, so it is minutes; \
               the report is wanted on demand rather than on every push",
    },
    Job {
        name: "msrv",
        // Not a task: the point of the job is to build with the oldest
        // supported toolchain, which `cargo make` cannot express because the
        // toolchain is pinned in `rust-toolchain.toml`. Named as the two
        // commands it runs so the "every named task exists" check still holds.
        tasks: &["check", "test-nextest"],
        in_local_gate: false,
        note: "builds on the MSRV rather than the pinned toolchain, which is a \
               different thing from anything the local gate can do",
    },
    Job {
        name: "desktop",
        tasks: &["desktop"],
        in_local_gate: false,
        note: "needs the fetched assets and a dx build, so it cannot run on a \
               bare checkout",
    },
    Job {
        name: "docker",
        tasks: &["docker-build", "docker-verify"],
        in_local_gate: false,
        note: "minutes, and it needs a docker daemon; it is a deploy-time \
               check rather than a code check",
    },
];

/// Checks that run locally on purpose and are not expected in CI.
const LOCAL_ONLY: &[(&str, &str)] = &[
    (
        "ci-fast",
        "the pre-push gate: the same checks without the supply-chain ones, which \
         are the slowest and only change when a dependency does",
    ),
    (
        "opt-levels",
        "the release optimisation level, which is declared in three places and \
         only one builds",
    ),
    (
        "setup",
        "installs the tools; not a check, and a CI job that ran it would be \
         reinstalling what the workflow already pinned",
    ),
    (
        "ci-slow",
        "mutation testing and the desktop build: minutes rather than seconds, so \
         they are opt-in locally",
    ),
];

#[test]
fn the_mapping_covers_every_ci_job() -> Result<()> {
    let jobs = ci_jobs(&read(".github/workflows/ci.yml")?)?;
    let mapped: BTreeSet<&str> = MAPPING.iter().map(|job| job.name).collect();
    let unmapped: Vec<&String> = jobs
        .iter()
        .filter(|job| !mapped.contains(job.as_str()))
        .collect();

    assert!(
        unmapped.is_empty(),
        "CI has jobs {unmapped:?} that `MAPPING` does not cover. Add a row, or \
         the gate is silently unenforced locally."
    );
    assert!(
        !mapped.is_empty(),
        "MAPPING is empty, so this test proves nothing"
    );
    Ok(())
}

#[test]
fn every_mapped_task_exists() -> Result<()> {
    let defined = all_tasks(&makefiles()?);
    let mut missing = Vec::new();
    for job in MAPPING {
        for task in job.tasks {
            if !defined.contains(*task) {
                missing.push(format!("{} -> {task}", job.name));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "MAPPING names tasks the makefiles do not define: {missing:?}"
    );
    Ok(())
}

#[test]
fn every_ci_job_runs_a_task_that_exists() -> Result<()> {
    // The workflow is the one place a typo becomes a red build nobody can
    // explain, because the job fails with "task not found" rather than with
    // anything about the change.
    let makefiles = makefiles()?;
    let defined = all_tasks(&makefiles);
    let yaml = read(".github/workflows/ci.yml")?;

    let mut bad = Vec::new();
    for job in ci_jobs(&yaml)? {
        for task in tasks_a_job_runs(&yaml, &job) {
            if !defined.contains(&task) {
                bad.push(format!("{job} runs `./mk {task}`, which does not exist"));
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    Ok(())
}

#[test]
fn every_ci_job_runs_something() -> Result<()> {
    // A job that runs no task is a job that nothing holds to the local gate,
    // whatever its name says.
    //
    // `msrv` is the one legitimate exception and it has to be named here: it
    // builds with the *oldest* supported toolchain, while `./mk` inherits the
    // pinned one from `rust-toolchain.toml`, so it spells its two commands out
    // instead. An exception that is not written down is an exception that
    // nobody is checking, which is what `MAPPING` is for everywhere else.
    const EXEMPT: &[&str] = &["msrv"];

    let yaml = read(".github/workflows/ci.yml")?;
    let mut silent = Vec::new();
    for job in ci_jobs(&yaml)? {
        if !EXEMPT.contains(&job.as_str()) && tasks_a_job_runs(&yaml, &job).is_empty() {
            silent.push(job);
        }
    }
    assert!(
        silent.is_empty(),
        "these CI jobs run no `./mk` task, so nothing holds them to the local \
         gate: {silent:?}. Either give it a task or add it to EXEMPT with a \
         reason."
    );
    Ok(())
}

#[test]
fn the_local_gate_runs_exactly_the_mapped_local_jobs() -> Result<()> {
    // The `ci` job runs `./mk ci`, and `ci` is a fan-out over the leaf
    // checks. So the comparison is between what `ci` *depends on* and the union
    // of the local rows: the leaf tasks for the `ci` row (which is the gate
    // itself), and the named tasks for every other local row.
    let makefiles = makefiles()?;
    let mut expected: BTreeSet<String> = MAPPING
        .iter()
        .filter(|job| job.in_local_gate)
        .flat_map(|job| {
            // A row whose single task is the gate expands to that gate's
            // dependencies; anything else is a literal task name.
            if job.tasks == ["ci"] {
                task_dependencies(&makefiles, "ci")
                    .unwrap_or_default()
                    .into_iter()
                    .collect::<Vec<_>>()
            } else {
                job.tasks.iter().map(|t| (*t).to_string()).collect()
            }
        })
        .chain(LOCAL_ONLY.iter().map(|(name, _)| (*name).to_string()))
        .collect();
    // `ci-fast`, `ci-slow` and `setup` are aggregate tasks: they are the
    // pre-push and one-off entry points, not leaf checks, and they are *run*
    // rather than *depended on*. They belong in the gate's own vocabulary, not
    // in the set of leaves it depends on.
    for aggregate in ["ci-fast", "ci-slow", "setup"] {
        expected.remove(aggregate);
    }

    let actual = task_dependencies(&makefiles, "ci")?;
    let missing: Vec<&String> = expected.difference(&actual).collect();
    let extra: Vec<&String> = actual.difference(&expected).collect();

    assert!(
        missing.is_empty() && extra.is_empty(),
        "`./mk ci` and MAPPING disagree. In MAPPING but not in `ci`: \
         {missing:?}. In `ci` but not accounted for: {extra:?}. Add a row to \
         MAPPING or LOCAL_ONLY rather than editing the task alone."
    );
    Ok(())
}

#[test]
fn ci_fast_is_a_subset_of_ci() -> Result<()> {
    // The pre-push hook is `ci-fast`, and a check that is in the pre-push gate
    // but not in CI means a contributor is paying for something CI never
    // checks. The reverse is the design: CI is a superset.
    let makefiles = makefiles()?;
    let full = task_dependencies(&makefiles, "ci")?;
    let fast = task_dependencies(&makefiles, "ci-fast")?;
    let extra: Vec<&String> = fast.difference(&full).collect();
    assert!(
        extra.is_empty(),
        "`ci-fast` runs {extra:?}, which `ci` does not. CI is meant to be the \
         superset; a check only in the pre-push gate is one CI never runs."
    );
    Ok(())
}

#[test]
fn the_slow_jobs_are_reachable_and_documented() -> Result<()> {
    let makefiles = makefiles()?;
    let slow = task_dependencies(&makefiles, "ci-slow")?;
    for job in MAPPING.iter().filter(|job| !job.in_local_gate) {
        assert!(
            !job.note.is_empty(),
            "{} is out of the local gate with no stated reason",
            job.name
        );
    }
    // `ci-slow` is referenced by the comments and by the docs, so it has to
    // exist; whether a given slow job is in it is a judgement about cost, not
    // drift, so it is asserted to be non-empty rather than to match.
    assert!(
        !slow.is_empty(),
        "`ci-slow` is referenced by the comments and must run something"
    );
    assert!(
        all_tasks(&makefiles).contains("ci-slow"),
        "`ci-slow` must exist: it is what the comments and CONTRIBUTING point at"
    );
    Ok(())
}

#[test]
fn every_hook_points_at_a_task_that_exists() -> Result<()> {
    let defined = all_tasks(&makefiles()?);
    let missing: Vec<String> = hooked_tasks(&read("prek.toml")?)
        .into_iter()
        .filter(|t| !defined.contains(t))
        .collect();
    assert!(
        missing.is_empty(),
        "prek.toml delegates to {missing:?}, which the makefiles do not define. \
         Every hook fails on push until the task is restored or the hook is cut."
    );
    Ok(())
}

#[test]
fn every_gated_task_has_a_hook() -> Result<()> {
    let makefiles = makefiles()?;
    let hooks = read("prek.toml")?;
    let covered = covered_by_a_hook(&hooks);
    let ci = task_dependencies(&makefiles, "ci")?;
    let missing: Vec<String> = ci.into_iter().filter(|t| !covered.contains(t)).collect();
    assert!(
        missing.is_empty(),
        "`./mk ci` runs {missing:?} with no prek hook. Add one at the \
         stage matching its cost, so the failure happens before the push."
    );
    Ok(())
}

#[test]
fn the_exceptions_are_all_real() -> Result<()> {
    // A stale entry in either list is how the next genuine drift gets waved
    // through, so both directions are asserted.
    let makefiles = makefiles()?;
    let defined = all_tasks(&makefiles);
    let run = task_dependencies(&makefiles, "ci")?;
    let mapped: BTreeSet<&str> = MAPPING
        .iter()
        .flat_map(|job| job.tasks.iter().copied())
        .collect();

    for (task, why) in LOCAL_ONLY {
        assert!(!why.is_empty(), "LOCAL_ONLY names `{task}` with no reason");
        assert!(
            !mapped.contains(task),
            "LOCAL_ONLY names `{task}`, which a CI job covers"
        );
        assert!(
            defined.contains(*task),
            "LOCAL_ONLY names `{task}`, which the makefiles do not define"
        );
        if !AGGREGATES.contains(task) {
            assert!(
                run.contains(*task),
                "LOCAL_ONLY names `{task}`, which `./mk ci` does not run"
            );
        }
    }
    Ok(())
}

#[test]
fn a_job_that_calls_a_tool_installs_it() -> Result<()> {
    // The runner does not have this repository's tools. A job that calls one
    // without installing it dies at that step, and the failure is a 127 rather
    // than anything about the change under test.
    //
    // The toolchain is deliberately absent from this list: it comes from
    // `rust-toolchain.toml` through `dtolnay/rust-toolchain`, and naming it
    // here would mean a job could pass with one toolchain and fail with
    // another.
    // Every job that runs a `cargo make` task must install `cargo-make`, and
    // every job that calls a tool directly must install that tool too. The
    // install line is a `taiki-e/install-action` `tool:` entry.
    //
    // Matched without the `@version`. The workflow installs these unpinned, so
    // `cargo-make` is the entry and `cargo-make@0.37.24` would also be one; this
    // checks that the tool is installed at all, which is the part that decides
    // whether the job runs. Which version it resolves to is install-action's
    // business, and pinning it here is a second place to update.
    const TOOLS: &[(&str, &str)] = &[
        ("./mk ", "cargo-make"),
        ("cargo nextest ", "cargo-nextest"),
        ("dx build", "dioxus-cli"),
        ("cargo mutants", "cargo-mutants"),
    ];

    let yaml = read(".github/workflows/ci.yml")?;
    let mut undeclared = Vec::new();
    for job in ci_jobs(&yaml)? {
        let block: String = yaml
            .lines()
            .skip_while(|l| *l != format!("  {job}:"))
            .skip(1)
            .take_while(|l| l.starts_with("   ") || l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        for (invocation, install) in TOOLS {
            if block.contains(invocation) && !block.contains(install) {
                undeclared.push(format!("{job} calls `{invocation}` with no `{install}`"));
            }
        }
    }
    assert!(
        undeclared.is_empty(),
        "these jobs invoke a tool the runner does not have:\n  {}",
        undeclared.join("\n  ")
    );
    Ok(())
}

#[test]
fn the_declarations_are_not_vacuous() -> Result<()> {
    // So a scraper that silently finds nothing cannot pass everything.
    let makefiles = makefiles()?;
    let ci = task_dependencies(&makefiles, "ci")?;
    assert!(ci.len() >= 12, "`./mk ci` looks truncated: {ci:?}");
    assert!(MAPPING.len() >= 5, "MAPPING looks truncated");
    assert!(
        all_tasks(&makefiles).len() >= 40,
        "the makefiles look truncated: {} tasks",
        all_tasks(&makefiles).len()
    );
    assert!(
        hooked_tasks(&read("prek.toml")?).len() >= 12,
        "prek.toml looks truncated"
    );
    assert!(
        ci_jobs(&read(".github/workflows/ci.yml")?)?.len() >= 4,
        "the CI scrape found too few jobs"
    );
    Ok(())
}
