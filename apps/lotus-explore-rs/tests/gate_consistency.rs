// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The local gate, the git hooks and the CI gate must be the same gate.
//!
//! `just ci`, `prek.toml` and `.github/workflows/ci.yml` are three hand-maintained
//! lists of the same checks, and hand-maintained lists drift. They had: `just ci`
//! ran checks CI did not, CI ran a job `just ci` did not, four gated recipes had
//! no hook at all, and the desktop job had no local equivalent -- which is the job
//! that would have caught the unstyled window, the no-op download and the missing
//! structure toolkit.
//!
//! `MAPPING` below is the correspondence, written out once. These tests hold it to
//! three things: it covers every CI job, every recipe it names really exists, and
//! `just ci` runs exactly the part of it meant to be local. Adding a check to one
//! file and forgetting the others now fails the build.
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

/// One CI job, the local recipes that cover it, and whether it gates.
struct Job {
    name: &'static str,
    /// Recipes whose combined commands are what this job runs.
    recipes: &'static [&'static str],
    /// `false` for the jobs `just ci` deliberately leaves to `just ci-slow`.
    in_local_gate: bool,
    /// Why, for a job that is not in the local gate.
    note: &'static str,
}

/// The correspondence, in CI's order.
const MAPPING: &[Job] = &[
    Job {
        name: "fmt",
        recipes: &["fmt"],
        in_local_gate: true,
        note: "",
    },
    Job {
        name: "license-headers",
        recipes: &["license-headers"],
        in_local_gate: true,
        note: "",
    },
    Job {
        name: "clippy",
        recipes: &["clippy", "clippy-native-builds"],
        in_local_gate: true,
        note: "",
    },
    Job {
        name: "test",
        recipes: &["test"],
        in_local_gate: true,
        note: "",
    },
    Job {
        name: "doc",
        recipes: &["doc"],
        in_local_gate: true,
        note: "",
    },
    Job {
        name: "wasm",
        recipes: &["wasm", "clippy-wasm"],
        in_local_gate: true,
        note: "",
    },
    Job {
        name: "machete",
        recipes: &["machete"],
        in_local_gate: true,
        note: "",
    },
    Job {
        name: "supply-chain",
        recipes: &["deny", "audit"],
        in_local_gate: true,
        note: "",
    },
    Job {
        name: "mutants",
        recipes: &["mutants"],
        in_local_gate: false,
        note: "non-blocking in CI: survivors are a to-do list, and it takes ~4 min",
    },
    Job {
        name: "desktop",
        recipes: &["desktop"],
        in_local_gate: false,
        note: "needs the fetched assets and a dx build, so it cannot run on a bare checkout",
    },
];

/// Checks that run locally on purpose and are not expected in CI.
const LOCAL_ONLY: &[(&str, &str)] = &[
    (
        "check",
        "a bare `cargo check`; the lints that follow already build everything",
    ),
    (
        "metadata",
        "workspace repository metadata, which `cargo package` needs and CI does not do",
    ),
    (
        "opt-levels",
        "the release optimisation level, which is declared in three places and only one builds",
    ),
];

/// The recipes `just ci` runs, in order.
fn ci_recipes(justfile: &str) -> Vec<String> {
    let body = justfile
        .split_once("\nci:\n")
        .map(|(_, rest)| rest)
        .unwrap_or_default()
        .lines()
        .take_while(|line| line.starts_with('\t') || line.trim().is_empty());
    body.filter_map(|line| line.trim().strip_prefix("just ").map(str::to_string))
        .collect()
}

/// Every recipe name defined in the justfile.
fn all_recipes(justfile: &str) -> BTreeSet<String> {
    justfile
        .lines()
        .filter_map(|line| {
            // A recipe starts at column zero, is `name:`, and is not a comment.
            if line.is_empty() || line.starts_with([' ', '\t', '#']) {
                return None;
            }
            let (name, _) = line.split_once(':')?;
            let name = name.trim();
            if name.is_empty() || name.contains(char::is_whitespace) {
                return None;
            }
            Some(name.to_string())
        })
        .collect()
}

/// The recipes the git hooks delegate to.
///
/// Every hook is `just <recipe>` with no cargo flags of its own, so the flags
/// cannot drift -- that is the point of delegating. What can still drift is the
/// *name*: a renamed or deleted recipe leaves the hook pointing at nothing and
/// failing on every push, and a new check can go into `just ci` with no hook to
/// catch it before the commit lands.
fn hooked_recipes(hooks: &str) -> BTreeSet<String> {
    let mut recipes = BTreeSet::new();
    for line in hooks.lines() {
        if let Some((_, rest)) = line.split_once("entry = \"just ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            if !name.is_empty() {
                recipes.insert(name);
            }
        }
    }
    recipes
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
        "CI has jobs {unmapped:?} that `MAPPING` does not cover. Add a row, or the \
         gate is silently unenforced locally."
    );
    assert!(
        !mapped.is_empty(),
        "MAPPING is empty, so this test proves nothing"
    );
    Ok(())
}

#[test]
fn every_mapped_recipe_exists() -> Result<()> {
    let defined = all_recipes(&read("justfile")?);
    let mut missing = Vec::new();
    for job in MAPPING {
        for recipe in job.recipes {
            if !defined.contains(*recipe) {
                missing.push(format!("{} -> {recipe}", job.name));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "MAPPING names recipes the justfile does not define: {missing:?}"
    );
    Ok(())
}

#[test]
fn just_ci_runs_exactly_the_mapped_local_gate() -> Result<()> {
    let expected: BTreeSet<String> = MAPPING
        .iter()
        .filter(|job| job.in_local_gate)
        .flat_map(|job| job.recipes.iter().map(|r| (*r).to_string()))
        .chain(LOCAL_ONLY.iter().map(|(name, _)| (*name).to_string()))
        .collect();

    let actual: BTreeSet<String> = ci_recipes(&read("justfile")?).into_iter().collect();
    let missing: Vec<&String> = expected.difference(&actual).collect();
    let extra: Vec<&String> = actual.difference(&expected).collect();

    assert!(
        missing.is_empty() && extra.is_empty(),
        "`just ci` and MAPPING disagree. Missing from `just ci`: {missing:?}. \
         In `just ci` but not accounted for: {extra:?}. Add a row to MAPPING or \
         LOCAL_ONLY rather than editing the recipe alone."
    );
    Ok(())
}

#[test]
fn the_slow_jobs_are_reachable_and_documented() -> Result<()> {
    let justfile = read("justfile")?;
    let in_fast_gate = ci_recipes(&justfile);
    for job in MAPPING.iter().filter(|job| !job.in_local_gate) {
        assert!(
            !job.note.is_empty(),
            "{} is out of the local gate with no stated reason",
            job.name
        );
        for recipe in job.recipes {
            assert!(
                justfile.contains(&format!("\tjust {recipe}\n")),
                "`{recipe}` is not run by `just ci-slow`, so {} can only be run by hand",
                job.name
            );
            assert!(
                !in_fast_gate.contains(&recipe.to_string()),
                "`{recipe}` is in `just ci` but MAPPING marks it slow"
            );
        }
    }
    Ok(())
}

#[test]
fn every_hook_points_at_a_recipe_that_exists() -> Result<()> {
    let defined = all_recipes(&read("justfile")?);
    let missing: Vec<String> = hooked_recipes(&read("prek.toml")?)
        .into_iter()
        .filter(|r| !defined.contains(r))
        .collect();
    assert!(
        missing.is_empty(),
        "prek.toml delegates to {missing:?}, which the justfile does not define. \
         Every hook fails on push until the recipe is restored or the hook is cut."
    );
    Ok(())
}

#[test]
fn every_gated_recipe_has_a_hook() -> Result<()> {
    let hooked = hooked_recipes(&read("prek.toml")?);
    let mut missing: Vec<String> = ci_recipes(&read("justfile")?)
        .into_iter()
        .filter(|r| !hooked.contains(r))
        .collect();
    // `mutants` and `desktop` are `just ci-slow`, and a dx build has no business
    // running on someone's commit.
    missing.retain(|r| !["mutants", "desktop"].contains(&r.as_str()));
    assert!(
        missing.is_empty(),
        "`just ci` runs {missing:?} with no prek hook. Add one at the stage matching \
         its cost, so the failure happens before the push."
    );
    Ok(())
}

#[test]
fn the_exceptions_are_all_real() -> Result<()> {
    // A stale entry in either list is how the next genuine drift gets waved
    // through, so both directions are asserted.
    let justfile = read("justfile")?;
    let run = ci_recipes(&justfile);
    let mapped: BTreeSet<&str> = MAPPING
        .iter()
        .flat_map(|job| job.recipes.iter().copied())
        .collect();

    for (recipe, why) in LOCAL_ONLY {
        assert!(
            !why.is_empty(),
            "LOCAL_ONLY names `{recipe}` with no reason"
        );
        assert!(
            run.contains(&recipe.to_string()),
            "LOCAL_ONLY names `{recipe}`, which `just ci` does not run"
        );
        assert!(
            !mapped.contains(recipe),
            "LOCAL_ONLY names `{recipe}`, which a CI job covers"
        );
        assert!(
            all_recipes(&justfile).contains(*recipe),
            "LOCAL_ONLY names `{recipe}`, which the justfile does not define"
        );
    }
    Ok(())
}

/// Each job's block of the workflow, as `(name, text)`.
///
/// Scanned rather than parsed: the app crate has no YAML parser, and what the
/// checks below need is "which lines belong to which job", which is decided by
/// indentation. A job key is at column 2; every key inside one is deeper.
fn ci_jobs_raw() -> Result<Vec<(String, String)>> {
    let yaml = read(".github/workflows/ci.yml")?;
    let mut jobs: Vec<(String, Vec<String>)> = Vec::new();

    for line in yaml.lines() {
        let at_column_two = line.starts_with("  ")
            && !line[2..].starts_with([' ', '\t'])
            && line.ends_with(':')
            && !line[2..].trim_end_matches(':').is_empty();
        if at_column_two {
            jobs.push((
                line[2..].trim_end_matches(':').to_owned(),
                vec![line.to_owned()],
            ));
        } else if let Some((_, lines)) = jobs.last_mut() {
            lines.push(line.to_owned());
        }
    }

    Ok(jobs
        .into_iter()
        .map(|(name, lines)| (name, lines.join("\n")))
        .collect())
}

#[test]
fn a_job_that_calls_a_tool_installs_it() -> Result<()> {
    // `just` is not on the GitHub runner. The desktop job has run
    // `just verify-bundle` since it was written and never once got as far as it:
    // the job died at link time first, so the missing tool was invisible until
    // the link was fixed. The mutants job inherited the same gap.
    //
    // So: any job whose steps invoke a tool has to have a step that installs it.
    // Tools this workflow shells out to, and the string that installs each.
    const TOOLS: &[(&str, &str)] = &[
        ("just ", "tool: just"),
        ("dx ", "tool: dioxus-cli"),
        ("cargo mutants", "tool: cargo-mutants"),
    ];

    let mut undeclared = Vec::new();
    for (name, block) in ci_jobs_raw()? {
        for (invocation, install) in TOOLS {
            let calls = block.lines().any(|line| {
                line.trim_start().starts_with("run:") || line.trim_start().starts_with("- ")
            }) && block.contains(invocation);
            if calls && !block.contains(install) {
                undeclared.push(format!("{name} calls `{invocation}` with no `{install}`"));
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
    let justfile = read("justfile")?;
    let run = ci_recipes(&justfile);
    assert!(run.len() >= 12, "`just ci` looks truncated: {run:?}");
    assert!(MAPPING.len() >= 8, "MAPPING looks truncated");
    assert!(
        all_recipes(&justfile).contains("ci-slow"),
        "`just ci-slow` is referenced by the comments and must exist"
    );
    assert!(
        hooked_recipes(&read("prek.toml")?).len() >= 15,
        "prek.toml looks truncated"
    );
    assert!(
        ci_jobs(&read(".github/workflows/ci.yml")?)?.len() >= 8,
        "the CI scrape found too few jobs"
    );
    Ok(())
}
