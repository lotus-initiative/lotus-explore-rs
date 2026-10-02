// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The CI workflow must install what it runs, and the tools it pins must agree
//! with the ones the local gate uses.
//!
//! A job that calls a tool the runner does not have dies at that step with a 127
//! and a message about a missing command, which says nothing about the change
//! under test. Three separate `install-action` bugs have been found this way: a
//! version range the action could not parse, a tool it could not resolve, and a
//! `#` comment inside `tool:` that it read as a tool name.

use super::scrapers::{Result, ci_jobs, read};

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The tombi version is written down once, in `prek.toml`, and read from there.
    ///
    /// `prek.toml` claims to be the single source for it, and `./mk setup` already
    /// reads it from that line rather than repeating the number. CI does not: its
    /// `setup-tombi` step names the version inline. So the claim holds for a
    /// contributor and not for a runner, which is the same split this file exists to
    /// catch everywhere else -- the hook formats with one tombi and the gate checks
    /// with another.
    ///
    /// This reads the version out of each and compares, rather than asserting one
    /// literal, so bumping `prek.toml` and forgetting CI fails here instead of
    /// silently linting with a different tool than the one that formatted the code.
    #[test]
    fn ci_installs_the_tombi_version_the_hook_uses() -> Result<()> {
        let hooks = read("prek.toml")?;
        let ci = read(".github/workflows/ci.yml")?;

        // The `rev` on the tombi-pre-commit repo, as a bare version.
        let hook_version = hooks
            .lines()
            .zip(hooks.lines().skip(1))
            .find(|(repo, _)| repo.contains("tombi-toml/tombi-pre-commit"))
            .and_then(|(_, next)| next.trim().strip_prefix("rev = \"v"))
            .and_then(|quoted| quoted.strip_suffix('"'))
            .map(str::to_string)
            .ok_or("prek.toml has no tombi-pre-commit rev")?;

        // The `version:` input on the setup-tombi step.
        let ci_version = ci
            .lines()
            .skip_while(|line| !line.contains("setup-tombi"))
            .find_map(|line| {
                line.trim()
                    .strip_prefix("version: \"")
                    .and_then(|quoted| quoted.strip_suffix('"'))
                    .map(str::to_string)
            })
            .ok_or("ci.yml has no setup-tombi step with a version")?;

        assert_eq!(
            hook_version, ci_version,
            "the hook runs tombi {hook_version} and CI runs {ci_version}: the gate \
         lints with a different tool than the one that formatted the code. \
         `prek.toml` is the single source -- change it and `ci.yml`, not just one."
        );

        // The pinned action SHA must be the tag's own commit, or the action and the
        // binary it installs are two different versions again.
        let pinned = ci
            .lines()
            .find_map(|line| line.split("setup-tombi@").nth(1))
            .map(|rest| {
                rest.split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string()
            })
            .ok_or("ci.yml does not pin setup-tombi to a commit")?;
        assert_eq!(
            pinned.len(),
            40,
            "setup-tombi is pinned to {pinned:?}, which is not a 40-character commit"
        );
        Ok(())
    }
}

/// The toolchain version is written down once per consumer, and they agree.
///
/// `rust-toolchain.toml` is the real pin. The Dockerfile cannot read it -- there
/// is no way to read a file before the first `FROM` -- so it repeats the number
/// in an `ARG`, and `compose.yaml` and the CI jobs repeat it again. Five places,
/// nothing enforcing that they match, and a contributor who bumps one has
/// shipped a local build on one toolchain and an image on another.
///
/// The Dockerfile comment claims this is already checked. It was not.
#[test]
fn every_toolchain_pin_agrees_with_rust_toolchain_toml() -> Result<()> {
    let pinned = read("rust-toolchain.toml")?
        .lines()
        .find_map(|line| line.trim().strip_prefix("channel = \""))
        .and_then(|quoted| quoted.strip_suffix('"'))
        .ok_or("rust-toolchain.toml has no [toolchain] channel")?
        .to_owned();

    // `ARG RUST_VERSION=1.99.0`
    let dockerfile = read("Dockerfile")?;
    let in_dockerfile = dockerfile
        .lines()
        .find_map(|line| line.trim().strip_prefix("ARG RUST_VERSION="))
        .map(|rest| rest.split('#').next().unwrap_or(rest).trim())
        .ok_or("the Dockerfile has no ARG RUST_VERSION")?;
    assert_eq!(
        in_dockerfile, pinned,
        "the Dockerfile builds on {in_dockerfile} and the repository pins {pinned}. \
         A local `dx build` and the container would not be the same toolchain."
    );

    // The comment directly above the ARG names the version; it drifts silently.
    // Only the four lines before the ARG are considered, so this is the comment
    // that is about this line rather than any comment in the file.
    let all_lines: Vec<&str> = dockerfile.lines().collect();
    let arg_at = all_lines
        .iter()
        .position(|line| line.trim_start().starts_with("ARG RUST_VERSION="))
        .ok_or("the Dockerfile has no ARG RUST_VERSION")?;
    let mut comment_above = None;
    for line in all_lines.iter().take(arg_at).rev().take(4) {
        if let Some(text) = line.trim().strip_prefix("# ")
            && text.contains("is the pin in rust-toolchain.toml")
        {
            comment_above = Some(text);
            break;
        }
    }
    let comment_above = comment_above.ok_or("the ARG has no comment above it naming the pin")?;

    assert!(
        comment_above.contains(&pinned),
        "the Dockerfile comment says {comment_above:?} but the ARG says {pinned}. \
         The comment is what a reader trusts when the two disagree."
    );

    // `RUST_VERSION: "1.99.0"` under the build service.
    let compose = read("compose.yaml")?;
    let in_compose = compose
        .lines()
        .find_map(|line| line.trim().strip_prefix("RUST_VERSION:"))
        .map(|rest| rest.trim_matches(|c: char| c == '"' || c.is_whitespace()))
        .ok_or("compose.yaml has no RUST_VERSION")?;
    assert_eq!(
        in_compose, pinned,
        "compose builds on {in_compose} and the repository pins {pinned}."
    );

    // Every non-MSRV job. The MSRV job deliberately builds with the oldest
    // supported toolchain, not the pinned one, so it is not in this set.
    let workflow = read(".github/workflows/ci.yml")?;
    for job in ci_jobs(&workflow)? {
        // Scoped to this job's own block. Scanning the whole file for every job
        // means the MSRV job's `toolchain: 1.97` is attributed to the `ci` job
        // as well, which is how the first version of this test reported a
        // mismatch that did not exist.
        let start = workflow
            .lines()
            .position(|line| line == format!("  {job}:"))
            .ok_or_else(|| format!("no block for job {job}"))?;
        let block: String = workflow
            .lines()
            .skip(start + 1)
            .take_while(|line| line.starts_with("   ") || line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        for line in block.lines() {
            let Some(toolchain) = line
                .trim()
                .strip_prefix("toolchain: ")
                .or_else(|| line.trim().strip_prefix("with: { toolchain: "))
                .and_then(|rest| rest.split([',', ' ']).next())
            else {
                continue;
            };
            if job == "msrv" {
                assert_ne!(
                    toolchain, pinned,
                    "the MSRV job must build with the oldest supported toolchain, not the pin"
                );
                continue;
            }
            assert_eq!(
                toolchain, pinned,
                "CI job `{job}` builds on {toolchain} and the repository pins {pinned}."
            );
        }
    }
    Ok(())
}
