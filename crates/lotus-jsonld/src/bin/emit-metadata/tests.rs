// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `emit-metadata`, in their own file.

use super::{ExitCode, Options, files, run, stale_files, write_files};
use std::path::Path;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|a| (*a).to_owned()).collect()
}

fn temp_root(tag: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("emit-metadata-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("temp dir");
    path
}

#[test]
fn check_is_off_unless_the_flag_is_there() {
    assert_eq!(
        Options::parse(&args(&[])),
        Options {
            check: false,
            root: ".".to_owned()
        },
        "no arguments means write into the current directory"
    );
    assert_eq!(
        Options::parse(&args(&["--check"])),
        Options {
            check: true,
            root: ".".to_owned()
        }
    );
}

#[test]
fn an_argument_that_is_not_the_flag_does_not_turn_checking_on() {
    // The test is an exact match, so a `--root=--check` style argument is
    // not mistaken for it.
    let options = Options::parse(&args(&["--root=/tmp/x"]));
    assert!(!options.check);
    assert_eq!(options.root, "/tmp/x");
}

#[test]
fn the_root_comes_from_the_prefixed_argument() {
    assert_eq!(Options::parse(&args(&["--root=/a/b"])).root, "/a/b");
    assert_eq!(
        Options::parse(&args(&["--check", "--root=/a/b"])),
        Options {
            check: true,
            root: "/a/b".to_owned()
        },
        "the two options are independent"
    );
    assert_eq!(
        Options::parse(&args(&["--root="])).root,
        "",
        "an empty root is as given"
    );
}

#[test]
fn a_root_is_matched_on_its_prefix_not_its_whole_text() {
    assert_eq!(
        Options::parse(&args(&["--root=/a", "--root=/b"])).root,
        "/a",
        "the first wins, so a later duplicate cannot move the output"
    );
}

#[test]
fn files_written_are_not_stale() {
    let root = temp_root("fresh");
    let files = files();
    write_files(&root, &files).expect("write");
    assert!(
        stale_files(&root, &files).is_empty(),
        "a run that just wrote them has nothing to report"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn only_the_file_that_differs_is_reported() {
    // One byte different in one file: exactly that file is named, so the
    // message points at the one to regenerate.
    let root = temp_root("stale-one");
    let generated = files();
    write_files(&root, &generated).expect("write");
    let (name, contents) = &generated[0];
    std::fs::write(root.join(name), format!("{contents} ")).expect("write");
    assert_eq!(stale_files(&root, &generated), vec![(*name).to_owned()]);
    let _ = std::fs::remove_dir_all(&root);

    // And the same for the other one, so the two are not symmetric by
    // accident of ordering.
    let root = temp_root("stale-two");
    let generated = files();
    write_files(&root, &generated).expect("write");
    let (name, contents) = &generated[1];
    std::fs::write(root.join(name), contents.trim_end()).expect("write");
    assert_eq!(stale_files(&root, &generated), vec![(*name).to_owned()]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_trailing_newline_is_enough_to_fail_the_check() {
    // The difference between a current file and one an editor touched.
    let root = temp_root("newline");
    let generated = files();
    write_files(&root, &generated).expect("write");
    let (name, contents) = &generated[0];
    std::fs::write(root.join(name), contents.trim_end()).expect("write");
    assert_eq!(
        stale_files(&root, &generated),
        vec![(*name).to_owned()],
        "whitespace is part of a generated file"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_missing_file_is_stale_rather_than_skipped() {
    // A file that cannot be read is the case `--check` exists for. Treating
    // it as "nothing to report" would make the check pass on an empty
    // directory, which is the opposite of what it is for.
    let root = temp_root("missing");
    assert_eq!(stale_files(&root, &files()).len(), 2);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn writing_reports_which_path_failed() {
    // The path is in the message, because the two generated files are
    // written from one run and a failure names only one of them.
    let root = temp_root("unwritable");
    let err = write_files(Path::new("/nonexistent-root-for-tests"), &files())
        .expect_err("writing under a missing directory fails");
    assert!(err.to_string().contains("codemeta.json"), "got {err}");
    let _ = std::fs::remove_dir_all(&root);
}

// The exit code is the whole contract of `--check` in CI: a stale file has
// to fail the build, and a current one has to pass it. `run` is tested
// directly because that code is what CI reads, not what it prints.

fn options(check: bool, root: &Path) -> Options {
    Options {
        check,
        root: root.display().to_string(),
    }
}

#[test]
fn checking_an_uncheckable_root_fails() {
    // Nothing written yet, so both files are stale.
    let root = temp_root("run-fresh");
    assert_eq!(run(&options(true, &root)), ExitCode::FAILURE);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn writing_then_checking_succeeds() {
    let root = temp_root("run-round-trip");
    assert_eq!(run(&options(false, &root)), ExitCode::SUCCESS);
    assert_eq!(run(&options(true, &root)), ExitCode::SUCCESS);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn checking_after_a_file_changed_fails() {
    let root = temp_root("run-edited");
    run(&options(false, &root));
    let generated = files();
    std::fs::write(root.join(generated[0].0), "edited").expect("write");
    assert_eq!(run(&options(true, &root)), ExitCode::FAILURE);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn writing_somewhere_unwritable_fails() {
    assert_eq!(
        run(&options(false, Path::new("/nonexistent-root-for-tests"))),
        ExitCode::FAILURE,
        "a generator that cannot write says so in its exit code, not only on stderr"
    );
}
