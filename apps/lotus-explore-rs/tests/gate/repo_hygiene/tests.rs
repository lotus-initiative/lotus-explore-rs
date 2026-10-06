// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The hygiene checks, in their own file.

use super::*;
use crate::gate::scrapers::repo_root;

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
                // The next non-blank line decides whether this is a module. A
                // `mod name;` is the declaration form and is the point; a braced
                // `mod name {` is the thing being banned.
                let next = lines
                    .iter()
                    .skip(number + 1)
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or(&"")
                    .trim();
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
