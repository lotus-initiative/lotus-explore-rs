// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The generator as CI runs it.
//!
//! `main` is one line, so its unit test is a test of `run`, which the module's
//! own tests cover. What they cannot reach is what CI depends on: the exit
//! status. A `--check` that writes nothing, prints the right message and exits 0
//! fails nothing, and that is the failure this file exists to rule out.

// An integration test links the crate's whole dependency graph without using it.
#![allow(
    unused_crate_dependencies,
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used
)]

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_emit-metadata");

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(tag: &str) -> Self {
        // Unique per process and per test, so the tests do not race each other
        // through a shared directory.
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "emit-metadata-cli-{tag}-{}-{unique}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("a temp root to write into");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(root: &Path, check: bool) -> std::process::Output {
    let mut command = Command::new(BIN);
    command.arg(format!("--root={}", root.display()));
    if check {
        command.arg("--check");
    }
    command.output().expect("the generator binary to run")
}

#[test]
fn writing_then_checking_exits_zero() {
    let root = TempRoot::new("fresh");

    let wrote = run(root.path(), false);
    assert!(
        wrote.status.success(),
        "writing failed: {}",
        String::from_utf8_lossy(&wrote.stderr)
    );
    assert!(
        root.path().join("codemeta.json").is_file(),
        "codemeta.json was written"
    );
    assert!(
        root.path().join("CITATION.cff").is_file(),
        "CITATION.cff was written"
    );

    let checked = run(root.path(), true);
    assert!(
        checked.status.success(),
        "--check failed straight after a write: {}",
        String::from_utf8_lossy(&checked.stderr)
    );
}

#[test]
fn checking_a_directory_with_nothing_in_it_fails() {
    // This is the case CI would silently pass if the exit code were wrong: no
    // files, no complaint on stdout, exit 0.
    let root = TempRoot::new("empty");
    let checked = run(root.path(), true);
    assert!(
        !checked.status.success(),
        "--check passed on an empty directory, so it gates nothing"
    );
    let stderr = String::from_utf8_lossy(&checked.stderr);
    assert!(
        stderr.contains("out of date"),
        "it says what to do: {stderr}"
    );
    assert!(
        stderr.contains("codemeta.json") && stderr.contains("CITATION.cff"),
        "and names both files: {stderr}"
    );
}

#[test]
fn checking_a_file_somebody_edited_fails_and_says_which() {
    let root = TempRoot::new("edited");
    run(root.path(), false);
    std::fs::write(root.path().join("codemeta.json"), "{}").expect("edit");

    let checked = run(root.path(), true);
    assert!(
        !checked.status.success(),
        "an edited file has to fail the check"
    );
    let stderr = String::from_utf8_lossy(&checked.stderr);
    assert!(
        stderr.contains("codemeta.json") && !stderr.contains("CITATION.cff"),
        "only the edited file is named: {stderr}"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("codemeta.json")).expect("read"),
        "{}",
        "--check reports, it does not repair"
    );
}

#[test]
fn an_argument_that_is_not_the_flag_does_not_turn_checking_on() {
    // Without --check the run writes. If the flag test were a substring match,
    // the `--root=` argument itself would enable checking and nothing would be
    // written.
    let root = TempRoot::new("nocheck");
    let wrote = run(root.path(), false);
    assert!(wrote.status.success());
    assert!(root.path().join("codemeta.json").is_file());
}

#[test]
fn a_root_that_cannot_be_written_to_fails_rather_than_pretending() {
    let output = run(Path::new("/nonexistent-root-for-emit-metadata"), false);
    assert!(
        !output.status.success(),
        "a generator that wrote nothing must not report success"
    );
    assert!(
        !String::from_utf8_lossy(&output.stderr).is_empty(),
        "and it has to say why"
    );
}
