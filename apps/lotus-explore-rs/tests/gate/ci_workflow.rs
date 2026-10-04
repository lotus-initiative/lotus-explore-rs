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

/// `ci.yml` and `./mk setup` install the same tools.
///
/// The versions are deliberately allowed to differ -- CI installs unpinned
/// while `setup.sh` pins -- but the tool *sets* are not. A tool in one list and
/// not the other means the local gate and the CI gate are different gates, and
/// this file exists to hold that one claim.
///
/// What this cannot catch, having tried: whether a name is a real crate.
/// `cargo-dejadoc` was in both lists for a commit, and both were wrong in the
/// same way, because the crate is `dejadoc` and that is the binary it installs.
/// There is no structural difference between that and `cargo-deny`, which is a
/// real crate with a `cargo-` prefixed name -- a rule rejecting `cargo-` names
/// fires on seven correct entries here. Only crates.io can tell them apart, and
/// a test that needs the network is not a gate.
#[cfg(test)]
mod installed_tools {
    use super::super::scrapers::{Result, read};
    use std::collections::{BTreeMap, BTreeSet};

    /// The entries of every `tool: |` block in the workflow.
    ///
    /// Read from the block rather than by filtering lines shaped like list
    /// items: the workflow is full of `- uses:` and `- name:` lines, and a shape
    /// guess picks those up. Indentation is the discriminator -- an entry in a
    /// literal block is indented past its own `tool: |` -- and the entries
    /// themselves are bare, because a YAML literal block is not a list.
    fn workflow_tools(yaml: &str) -> BTreeSet<String> {
        let mut tools = BTreeSet::new();
        let mut indent = 0usize;
        let mut in_block = false;
        for line in yaml.lines() {
            let width = line.len() - line.trim_start().len();
            let trimmed = line.trim();
            if trimmed == "tool: |" {
                in_block = true;
                indent = width;
                continue;
            }
            if !in_block {
                continue;
            }
            if trimmed.is_empty() || width <= indent {
                in_block = false;
            } else if !trimmed.starts_with('#') {
                tools.insert(crate_name(trimmed).to_string());
            }
        }
        tools
    }

    /// `install_tool <crate> <version>` from `setup.sh`, with the versions.
    fn setup_tools(script: &str) -> BTreeMap<String, String> {
        script
            .lines()
            .filter_map(|line| line.trim().strip_prefix("install_tool "))
            .filter_map(|rest| {
                let mut fields = rest.split_whitespace();
                let name = fields.next()?;
                let version = fields.next().unwrap_or("unpinned");
                Some((crate_name(name).to_string(), version.to_string()))
            })
            .collect()
    }

    /// A crate name, with any `@version` and trailing `:tag` ref removed.
    fn crate_name(entry: &str) -> &str {
        let entry = entry.split('#').next().unwrap_or(entry).trim();
        let entry = entry.rsplit_once(' ').map_or(entry, |(_, tail)| tail);
        entry.split('@').next().unwrap_or(entry).trim()
    }

    /// Tools CI installs that `setup.sh` deliberately does not, and why.
    ///
    /// Listed rather than derived, because each is a bootstrap: `setup.sh`
    /// cannot install them with the helper they bootstrap, or installs them
    /// from a pinned source instead.
    const SETUP_EXCEPTIONS: &[&str] = &[
        // The installer itself. `setup.sh` probes for it and falls back to
        // `cargo install` when it is absent, so it is never a target of
        // `install_tool`.
        "cargo-binstall",
        // Installed from the pinned version in Cargo.toml rather than through
        // `install_tool`, because the release binary and the crate are named
        // differently and the version is read from the manifest.
        "dioxus-cli",
    ];

    #[test]
    fn ci_installs_nothing_the_local_setup_cannot_provide() -> Result<()> {
        // One direction only. `setup.sh` is a superset: it installs the local
        // tools for tasks the CI gate does not run -- mutants, bloat, udeps --
        // and requiring the sets to match would be requiring CI to grow every
        // one of them.
        //
        // The other direction is the one that matters and that was previously
        // unchecked: a tool added to the CI gate without being installable
        // locally means `./mk ci` fails on a contributor's machine for a reason
        // that has nothing to do with their change.
        let in_ci = workflow_tools(&read(".github/workflows/ci.yml")?);
        let in_setup: BTreeSet<String> = setup_tools(&read("make/scripts/setup.sh")?)
            .into_keys()
            .collect();

        let unavailable: Vec<String> = in_ci
            .difference(&in_setup)
            .filter(|name| !SETUP_EXCEPTIONS.contains(&name.as_str()))
            .cloned()
            .collect();

        assert!(
            unavailable.is_empty(),
            "CI installs tools `./mk setup` does not provide, so the local gate \
             cannot run what CI runs:\n  {}\n\
             Add an `install_tool` line for each, or list it in \
             SETUP_EXCEPTIONS with the reason it is a bootstrap.",
            unavailable.join("\n  ")
        );
        Ok(())
    }

    /// The duplicate-doctest tool, named the way each consumer needs it.
    ///
    /// Written down because the two spellings differ and both had to be right:
    /// the installers take `dejadoc`, and the task runs `cargo-dejadoc`. Getting
    /// this backwards fails in CI with "cargo-dejadoc is not found", which names
    /// neither the crate nor the typo.
    #[test]
    fn the_duplicate_doctest_tool_is_named_correctly_in_both_places() -> Result<()> {
        let yaml = read(".github/workflows/ci.yml")?;
        assert!(
            workflow_tools(&yaml).contains("dejadoc"),
            "ci.yml installs the crate as `dejadoc`, not as `cargo-dejadoc`: \
             install-action takes crate names and `cargo-dejadoc` does not exist"
        );

        let script = read("make/scripts/setup.sh")?;
        let versions = setup_tools(&script);
        assert!(
            versions.contains_key("dejadoc"),
            "setup.sh installs it as `dejadoc` too, for the same reason"
        );

        let task = read("make/test.toml")?;
        assert!(
            task.contains("command -v cargo-dejadoc") && task.contains("cargo dejadoc"),
            "the task runs the *binary*, which really is `cargo-dejadoc` -- so the \
             crate name and the command name are not the same string, and both \
             spellings above have to be right for the gate to run"
        );
        Ok(())
    }
}
