// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Repository hygiene the gate can check cheaply.
//!
//! Both of these exist because something went wrong once and nothing failed: the
//! fetched asset trees were committed by a `git add -A` that swept 33,000 lines of
//! vendored third-party code into a commit, and a `fetch-assets` run from the
//! workspace root wrote a `public/` tree that nothing reads and nothing ignored.

use super::scrapers::Result;

#[cfg(test)]
mod tests {
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
}
