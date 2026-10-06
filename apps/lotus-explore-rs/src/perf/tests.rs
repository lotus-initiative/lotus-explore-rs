// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! A `console.time` label holds one timer, so one label must have one start.
//!
//! The bug this catches is not a crash. Opening the same label from a function
//! the caller has already opened it in makes the browser ignore the inner
//! `time()`, so the inner `timeEnd` closes the *outer* timer and both durations
//! come out wrong -- plausibly wrong, which is worse. It showed up as
//! `Timer "LOTUS:taxon_resolution" already exists.` in the console.
//!
//! Checked by reading the source rather than by running it, because the two
//! sites have to be on one call path for the collision to matter and that is
//! not something a unit test can arrange.

use super::Timer;
#[cfg(target_arch = "wasm32")]
use super::release_timer;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The label a call site passes, or `None` if the line is not a call.
///
/// Requires a `"` immediately after the `(`, which is what every real call
/// looks like and is not what this file's own scanning code looks like -- hence
/// also skipping the whole `perf` module below.
fn label_on<'a>(line: &'a str, call: &str) -> Option<&'a str> {
    let rest = line.split(call).nth(1)?.trim_start();
    rest.strip_prefix('"')?.split('"').next()
}

/// Every `.rs` file under `src/` except this module's, so a timer added anywhere
/// is covered without the scanner reading its own pattern-matching code.
fn sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, found);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                found.push(path);
            }
        }
    }
    let mut found = Vec::new();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    walk(&src, &mut found);
    // Two files, not one. `perf.rs` defines `start_timer`/`end_timer` and so
    // calls them with a variable label; these tests moved to `perf/tests.rs`, so
    // skipping only `perf.rs` left the scanner reading its own pattern-matching
    // code and reporting it as an ad-hoc call site.
    found.retain(|path| {
        let name = path.file_name().is_some_and(|n| n == "perf.rs");
        let scanner = path.file_name().is_some_and(|n| n == "tests.rs")
            && path.parent().is_some_and(|p| p.ends_with("perf"));
        !name && !scanner
    });
    found.sort();
    assert!(!found.is_empty(), "the source walk found nothing");
    found
}

#[test]
fn every_timer_label_is_started_from_exactly_one_place() {
    let mut starts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in sources() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for (index, line) in text.lines().enumerate() {
            let Some(label) = label_on(line, "start_timer(") else {
                continue;
            };
            starts.entry(label.to_owned()).or_default().push(format!(
                "{}:{}",
                path.file_name().unwrap_or_default().to_string_lossy(),
                index + 1
            ));
        }
    }

    assert!(!starts.is_empty(), "no timers found: the walk is wrong");

    let shared: Vec<String> = starts
        .iter()
        .filter(|(_, sites)| sites.len() > 1)
        .map(|(label, sites)| format!("{label} is started from {sites:?}"))
        .collect();

    assert!(
        shared.is_empty(),
        "a console.time label holds one timer, so two start sites can collide \\
         and each will report the other's duration:\\n  {}",
        shared.join("\\n  ")
    );
}

#[test]
fn every_started_label_is_also_ended() {
    // A label that is opened and never closed leaves a timer running forever,
    // and the next search to open it is the "already exists" case above.
    let text: String = sources()
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
    let mut started = BTreeMap::<String, usize>::new();
    let mut ended = BTreeMap::<String, usize>::new();
    for line in text.lines() {
        if let Some(label) = label_on(line, "start_timer(") {
            *started.entry(label.to_owned()).or_default() += 1;
        }
        if let Some(label) = label_on(line, "end_timer(") {
            *ended.entry(label.to_owned()).or_default() += 1;
        }
    }

    let unclosed: Vec<&String> = started
        .keys()
        .filter(|label| !ended.contains_key(*label))
        .collect();

    assert!(
        unclosed.is_empty(),
        "these labels are opened and never closed: {unclosed:?}"
    );
}

/// A label built at runtime cannot be checked by the two scanners above, which
/// only see string literals.
///
/// That is not a hypothetical gap. The download timers are
/// `format!("LOTUS:download_{format}")`, opened in `download.rs` and closed in
/// `download/wasm.rs` or `download/native.rs` — different files, so the pairing
/// is invisible to any per-label count, and dynamic, so the label is never named
/// twice in one place for the collision test to see either. Two downloads of the
/// same format in flight therefore collide exactly the way `resolve` did, and
/// nothing in this file can see it.
///
/// What *is* checkable is that the label is built by one of the two helpers rather
/// than formatted inline at a call site. Keeping the `format!` in one place per
/// label means two sites cannot spell the same label differently, which is the
/// failure this whole file is about.
#[test]
fn runtime_built_labels_go_through_the_shared_helpers() {
    let ad_hoc: Vec<String> = sources()
        .iter()
        .flat_map(|path| {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            text.lines()
                .enumerate()
                .filter(|(_, line)| {
                    (line.contains("start_timer(") || line.contains("end_timer("))
                        && !line.contains("start_timer(\"")
                        && !line.contains("end_timer(\"")
                        // `format!` inline at the call site: a label spelled here
                        // can drift from the same label spelled elsewhere.
                        && !line.contains("timer_label(")
                })
                .map(|(index, line)| {
                    format!(
                        "{}:{}: {}",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        index + 1,
                        line.trim()
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        ad_hoc.is_empty(),
        "build a runtime timer label in `export_timer_label` or \
         `export_trigger_timer_label`, not at the call site: {ad_hoc:#?}"
    );
}

#[test]
fn a_timer_reports_a_duration_and_cannot_be_read_twice() {
    // `end` consumes the guard, which is what keeps the returned duration and the
    // `Drop` close from both firing: `end` takes the handle, so there is nothing
    // left for `Drop` to close a second time.
    let elapsed = Timer::start("LOTUS:test_timer_guard").end();
    assert!(
        elapsed < Duration::from_secs(30),
        "a no-op test timer reported {elapsed:?}"
    );
}

#[test]
fn a_dropped_timer_is_closed_rather_than_leaked() {
    // The leak this pins is invisible on native, where `end_timer` reads the
    // handle and ignores the label, so native asserts only that dropping the
    // guard runs the close without panicking and does not close twice. On wasm
    // the label itself is observable, and that is where the assertions bite.
    let label = "LOTUS:test_timer_dropped";
    {
        let _timer = Timer::start(label);
        if let Some(open) = label_is_open(label) {
            assert!(open, "the guard should hold the label while it is in scope");
        }
    }
    if let Some(open) = label_is_open(label) {
        assert!(
            !open,
            "the guard must release its label when it goes out of scope, or every \
             later timer with this label finds it taken"
        );

        // And the label is genuinely reusable afterwards, which is the part that
        // was broken: a second open has to be a fresh timer, not the already-open
        // branch that skips `console.time` and reports the wrong duration.
        let _second = Timer::start(label);
        assert!(
            label_is_open(label).unwrap_or(true),
            "the label should be reusable once the guard has released it"
        );
    }
}

/// Whether `label` is currently held, on whichever target is being tested.
///
/// `Some` on wasm, where this is the real `OPEN_TIMERS` entry. `None` on native,
/// which has no global label namespace to ask about.
// `Option` is load-bearing across targets and redundant within either one: wasm
// answers from the real registry and native has no registry to ask. The lint reads
// only the wasm arm, where the `Option` really is always `Some`.
#[cfg(target_arch = "wasm32")]
#[allow(clippy::unnecessary_wraps)]
fn label_is_open(label: &str) -> Option<bool> {
    Some(!release_timer(label))
}

#[cfg(not(target_arch = "wasm32"))]
fn label_is_open(_label: &str) -> Option<bool> {
    None
}
