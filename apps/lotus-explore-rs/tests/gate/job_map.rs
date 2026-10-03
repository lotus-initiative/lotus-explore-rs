// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The correspondence between CI jobs, local tasks, and git hooks.
//!
//! `./mk ci`, `prek.toml` and `.github/workflows/ci.yml` are three
//! hand-maintained lists of the same checks, and hand-maintained lists drift.
//! They had: the local gate ran checks CI did not, CI ran a job the local gate
//! did not, four gated tasks had no hook at all, and the desktop job had no
//! local equivalent -- which is the job that would have caught the unstyled
//! window, the no-op download and the missing structure toolkit.

use super::scrapers::{
    Result, all_tasks, ci_jobs, covered_by_a_hook, hooked_tasks, makefiles, read,
    task_dependencies, tasks_a_job_runs,
};

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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::super::scrapers::AGGREGATES;
    use super::*;

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
        // `gate`, `ci-fast`, `ci-slow` and `setup` are aggregate tasks: they are the
        // inner loop, the pre-push and one-off entry points, not leaf checks, and they
        // are *run* rather than *depended on*. They belong in the gate's own
        // vocabulary, not in the set of leaves it depends on.
        //
        // `gate` being here is what lets it be a strict subset of `ci-fast` without
        // the test below reading the omission as an unexplained difference: it is an
        // entry point, and the leaves it names are still accounted for.
        for aggregate in ["gate", "ci-fast", "ci-slow", "setup"] {
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
}
