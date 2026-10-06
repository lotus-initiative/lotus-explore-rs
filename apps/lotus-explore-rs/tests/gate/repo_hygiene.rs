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
#[path = "repo_hygiene/tests.rs"]
mod tests;
