// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The hygiene checks, in their own file.

use super::*;
use crate::gate::scrapers::{read, repo_root};

#[test]
fn the_fetched_asset_trees_are_not_tracked() -> Result<()> {
    // Every tracked path, filtered here rather than by a `git ls-files` pathspec.
    // A pathspec is matched against the repository root, so `-- public/` from the
    // root means the root's own `public/` -- which is one of the trees being
    // checked for, not all of them. The first version of this test passed with a
    // real asset staged because the pathspec matched nothing and an empty result
    // reads as "clean".
    let tracked = std::process::Command::new("git")
        .args(["ls-files"])
        .current_dir(repo_root())
        .output()?;
    let listed = String::from_utf8_lossy(&tracked.stdout);
    let offenders: Vec<&str> = listed
        .lines()
        .filter(|path| {
            path.contains("/public/assets/ketcher/")
                || path.contains("/public/assets/vendor/")
                || path.starts_with("public/assets/ketcher/")
                || path.starts_with("public/assets/vendor/")
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "these fetched assets are tracked. They are downloaded, not source:\n  {}\n\
     Untrack them with `git rm -r --cached` on the paths above; the files on \
     disk are fine and are rebuilt by `fetch-assets`.",
        offenders.join("\n  ")
    );
    Ok(())
}

/// `.gitignore` covers the wrong-directory case, not just the right one.
///
/// The root `public/` is the tree `fetch-assets` creates when run from the
/// workspace root instead of the app crate. Nothing reads it, so it is easy to
/// create and easy to commit. The root `.gitignore` used to name only the app
/// path and the `lotus-web-assets` one, which left this hole open until a test
/// run wrote one.
#[test]
fn the_wrong_directory_asset_tree_is_ignored() -> Result<()> {
    let root = repo_root();
    for candidate in ["public", "public/assets/ketcher", "public/assets/vendor"] {
        let probe = root.join(candidate);
        if !probe.exists() {
            continue;
        }
        let out = std::process::Command::new("git")
            .args(["check-ignore", "-q", candidate])
            .current_dir(&root)
            .output()?;
        assert!(
            out.status.success(),
            "`{candidate}` exists and is not ignored. It was written by \
         `fetch-assets` running from the workspace root rather than from the \
         app crate, and committing it would put a third-party tree in the \
         repository. Add it to the root `.gitignore`."
        );
    }
    Ok(())
}

/// Every `#[cfg(test)]` module is a declaration, not an inline block.
///
/// A test file that carries its own tests is readable on its own, which is the
/// only argument for the rule, but the rule is here rather than left to taste
/// because the alternative decays silently: an inline `mod tests` in a 200-line
/// source file is invisible to review, and a 10,000-line one is worse. The
/// workspace had 132 of them when this was written, every one of them at least
/// 221 lines, so none of them was a module too small to have been extracted in
/// the first place.
///
/// The check walks the filesystem rather than shelling out to `git grep`,
/// because a brand-new file is untracked until it is added, and a check that
/// cannot see the file someone just wrote is a check that reports a clean tree
/// while the violation is still on the screen in front of them.
///
/// Excluded, and why each one has to be:
///   - `tests/` directories, which are integration tests and are already files;
///   - files *named* `tests.rs` or `*_tests.rs`, which are the destination of
///     this rule rather than a violation of it;
///   - `build.rs`, which is a separate crate root whose tests live in
///     `build/tests.rs` and which has no module tree to speak of.
#[test]
fn test_modules_live_in_their_own_files() -> Result<()> {
    let root = repo_root();
    let mut offenders = Vec::new();

    let mut stack = vec![root.join("crates"), root.join("apps")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if name == "target" || name == "tests" {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !name.to_ascii_lowercase().ends_with(".rs") {
                continue;
            }
            if name == "tests.rs" || name.ends_with("_tests.rs") || name == "build.rs" {
                continue;
            }

            let text = std::fs::read_to_string(&path)?;
            let lines: Vec<&str> = text.lines().collect();
            for (number, line) in lines.iter().enumerate() {
                if line.trim() != "#[cfg(test)]" {
                    continue;
                }
                // The next non-attribute, non-comment line decides whether this
                // is a module. A `mod name;` is the declaration form and is the
                // point; a braced `mod name {` is the thing being banned, even
                // when an `#[allow(...)]` sits between `#[cfg(test)]` and `mod`.
                let next = lines
                    .iter()
                    .skip(number + 1)
                    .map(|line| line.trim())
                    .find(|trimmed| {
                        !trimmed.is_empty()
                            && !trimmed.starts_with('#')
                            && !trimmed.starts_with("//")
                    })
                    .unwrap_or("");
                if next.starts_with("mod ") && next.contains('{') {
                    offenders.push(format!(
                        "{}:{}",
                        path.strip_prefix(&root).unwrap_or(&path).display(),
                        number + 2
                    ));
                }
            }
        }
    }
    offenders.sort();

    assert!(
        offenders.is_empty(),
        "these sites carry an inline `#[cfg(test)] mod`:\n  {}\n\
         Move each module into its own file and leave a declaration behind:\n\n  \
             #[cfg(test)]\n  \
             #[path = \"<module>/<name>.rs\"]\n  \
             mod <name>;\n\n\
         The `#[path]` is needed unless the parent is `lib.rs`, `main.rs`, \
         `build.rs` or `mod.rs`, because Rust looks for the children of `foo.rs` \
         under `foo/`. The new file needs the two AGPL SPDX lines, its own `//!` \
         doc comment, and `use super::*;` -- and the `//!` and any `#![...]` must \
          come *before* the first item, or the build fails on an inner attribute.",
        offenders.join("\n  ")
    );
    Ok(())
}

/// The root `AGENTS.md` is the canonical instruction file every agent looks for.
///
/// It was accidentally removed and gitignored at one point (commit `96010a0`),
/// leaving only a broken `GEMINI.md → AGENTS.md` symlink and a graphify-only
/// `.github/ai/AGENTS.md`. Without a root `AGENTS.md`, an agent cannot discover
/// the build/test/lint commands or the project conventions without first reading
/// README.md and CONTRIBUTING.md. This test fails if that file is removed again.
#[test]
fn the_root_instruction_file_exists() {
    let root = repo_root();
    let candidates = ["AGENTS.md", "CLAUDE.md", "GEMINI.md"];
    let found = candidates
        .iter()
        .any(|name| root.join(name).exists() && root.join(name).metadata().is_ok());
    assert!(
        found,
        "no root instruction file found. Every agent (Claude Code, OpenCode, Gemini \
         CLI, etc.) looks for {} at the repository root, and the project has none. \
         Create AGENTS.md with build/test/lint commands and key conventions. A \
         GEMINI.md symlink is not enough if it is broken.",
        candidates.join(", "),
    );
}

/// The root `AGENTS.md` mentions the key build/test/lint commands.
///
/// An AGENTS.md that exists but does not name `./mk ci` or `./mk gate` is a
/// file that will be read and then ignored, because the agent has to go hunting
/// elsewhere for the actual commands. This test reads the file and asserts the
/// four commands every contributor needs are named.
#[test]
fn the_root_instruction_file_names_the_key_commands() -> Result<()> {
    let content = read("AGENTS.md")?;
    for cmd in ["./mk ci", "./mk gate", "./mk setup", "./mk test"] {
        assert!(
            content.contains(cmd),
            "AGENTS.md does not mention `{cmd}`. Add it so an agent does not \
             have to learn the build system by reading Makefile.toml."
        );
    }
    Ok(())
}

/// The MCP config must not reference nonexistent packages.
///
/// `.github/ai/.mcp.json` is tracked in git and used by any MCP-capable agent.
/// A typo in a package name (`graphifyy` instead of `graphify`) makes the MCP
/// server silently unavailable, and nothing else catches it.
#[test]
fn the_mcp_config_has_no_typo_in_package_names() -> Result<()> {
    let content = read(".github/ai/.mcp.json")?;
    assert!(
        !content.contains("graphifyy"),
        ".github/ai/.mcp.json contains `graphifyy`, which is a typo for `graphify`. \
         `uvx --from graphifyy` cannot resolve the package and the MCP server \
         will not start."
    );
    Ok(())
}

/// Extract the pinned channel from `rust-toolchain.toml`.
fn rust_toolchain_channel() -> Result<String> {
    let content = read("rust-toolchain.toml")?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("channel")
            && let Some(val) = rest.trim_start().strip_prefix('=')
        {
            return Ok(val.trim().trim_matches('"').to_owned());
        }
    }
    Err("rust-toolchain.toml does not specify a `channel`".into())
}

#[test]
fn rust_toolchain_version_is_consistent_across_manifests_and_workflows() -> Result<()> {
    let version = rust_toolchain_channel()?;
    let checks = [
        ("Cargo.toml", format!("rust-version = \"{version}\"")),
        ("Dockerfile", format!("ARG RUST_VERSION={version}")),
        ("compose.yaml", format!("RUST_VERSION: \"{version}\"")),
        (
            "crates/lotus-jsonld/src/lib.rs",
            format!("requires_rust: \">= {version}\""),
        ),
        (".github/workflows/ci.yml", format!("toolchain: {version}")),
        (
            ".github/workflows/archive.yml",
            format!("toolchain: {version}"),
        ),
    ];
    for (file, expected) in checks {
        let content = read(file)?;
        assert!(
            content.contains(&expected),
            "{file} is out of sync with rust-toolchain.toml ({version}): expected `{expected}`"
        );
    }
    Ok(())
}

#[test]
fn security_policy_referenced_by_site_metadata_exists() {
    assert!(
        repo_root().join("SECURITY.md").is_file(),
        "SECURITY.md must exist at repository root because site-metadata.json \
         and security.txt link to it"
    );
}

#[test]
fn human_docs_never_link_to_agent_files() -> Result<()> {
    let root = repo_root();
    let mut files = vec![
        root.join("README.md"),
        root.join("CONTRIBUTING.md"),
        root.join("CHANGELOG.md"),
        root.join("SECURITY.md"),
        root.join(".github/PULL_REQUEST_TEMPLATE.md"),
    ];
    for dir in [root.join("docs"), root.join("crates"), root.join("apps")] {
        let mut stack = vec![dir];
        while let Some(current) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&current) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if path.file_name().is_some_and(|n| n == "target") {
                        continue;
                    }
                    stack.push(path);
                } else if path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
                {
                    files.push(path);
                }
            }
        }
    }

    let forbidden = ["AGENTS.md", "CLAUDE.md", "GEMINI.md", ".github/ai/"];
    for path in files {
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let rel = path.strip_prefix(&root).unwrap_or(&path);
        for token in forbidden {
            assert!(
                !text.contains(token),
                "{} mentions agent file `{token}` — human docs must stay independent of agent files",
                rel.display()
            );
        }
    }
    Ok(())
}

#[test]
fn session_log_is_absent() {
    assert!(
        !repo_root().join("SESSION_LOG.md").exists(),
        "SESSION_LOG.md is a local agent scratchpad and must not be committed"
    );
}

#[test]
fn crate_production_lint_allows_carry_a_reason() -> Result<()> {
    let root = repo_root();
    let mut stack = vec![root.join("crates")];
    let mut files = Vec::new();
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let norm = path.to_string_lossy().replace('\\', "/");
                if norm.contains("/src/")
                    && !norm.ends_with("/tests.rs")
                    && !norm.ends_with("_tests.rs")
                    && !norm.ends_with("/test_support.rs")
                {
                    files.push(path);
                }
            }
        }
    }

    let mut offenders = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path)?;
        let rel = path.strip_prefix(&root).unwrap_or(&path);
        for (line_no, attr) in extract_allow_attributes(&text) {
            if !attr.contains("reason =") && !attr.contains("reason=") {
                offenders.push(format!("{}:{line_no}: {attr}", rel.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "production lint `allow(...)` attributes in crates/ must declare `reason = \"...\"`:\n  {}",
        offenders.join("\n  ")
    );
    Ok(())
}

fn extract_allow_attributes(src: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0_usize;
    let mut line = 1_usize;

    while let Some(&b) = bytes.get(i) {
        if b == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        // Skip line comments so prose mentions like `#[allow(...)]` do not count.
        if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while let Some(&c) = bytes.get(i) {
                i += 1;
                if c == b'\n' {
                    line += 1;
                    break;
                }
            }
            continue;
        }
        // Skip string literals so raw strings in fixtures do not trigger.
        if b == b'"' {
            i += 1;
            while let Some(&c) = bytes.get(i) {
                if c == b'\\' {
                    i += 2;
                    continue;
                }
                if c == b'\n' {
                    line += 1;
                }
                i += 1;
                if c == b'"' {
                    break;
                }
            }
            continue;
        }
        let is_inner =
            b == b'#' && bytes.get(i + 1) == Some(&b'!') && bytes.get(i + 2) == Some(&b'[');
        let is_outer = b == b'#' && bytes.get(i + 1) == Some(&b'[');
        if is_inner || is_outer {
            let start = i;
            let start_line = line;
            let open_bracket = i + if is_inner { 2 } else { 1 };
            let mut depth = 1_usize;
            i = open_bracket + 1;
            while i < bytes.len() && depth > 0 {
                let Some(&c) = bytes.get(i) else {
                    break;
                };
                if c == b'"' {
                    i += 1;
                    while let Some(&sc) = bytes.get(i) {
                        if sc == b'\\' {
                            i += 2;
                            continue;
                        }
                        if sc == b'\n' {
                            line += 1;
                        }
                        i += 1;
                        if sc == b'"' {
                            break;
                        }
                    }
                    continue;
                }
                if c == b'[' {
                    depth += 1;
                } else if c == b']' {
                    depth -= 1;
                } else if c == b'\n' {
                    line += 1;
                }
                i += 1;
            }
            if let Some(attr) = src.get(start..i)
                && attr.contains("allow(")
            {
                let summary = attr.lines().map(str::trim).collect::<Vec<_>>().join(" ");
                out.push((start_line, summary));
            }
            continue;
        }
        i += 1;
    }
    out
}
