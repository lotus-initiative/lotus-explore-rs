// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Performance monitoring and logging infrastructure for SPARQL query execution.

use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

/// Timer handle - stores platform-specific timing data.
/// On WASM, this stores `performance.now()` milliseconds.
#[cfg(target_arch = "wasm32")]
pub type TimerHandle = f64;

#[cfg(not(target_arch = "wasm32"))]
pub type TimerHandle = Instant;

#[cfg(target_arch = "wasm32")]
fn wasm_now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}

/// Log a message with timing context. Works cross-platform (WASM console vs. native stdout).
pub fn log_timing(phase: &str, message: &str, duration: Option<Duration>) {
    if !log::log_enabled!(log::Level::Info) {
        return;
    }

    let msg = duration.map_or_else(
        || format!("[LOTUS:{phase}] {message}"),
        |d| {
            let ms = d.as_secs_f64() * 1000.0;
            format!("[LOTUS:{phase}] {message} ({ms:.1}ms)")
        },
    );

    #[cfg(target_arch = "wasm32")]
    {
        web_sys::console::info_1(&msg.into());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        log::info!("{msg}");
    }
}

/// The `console.time()` labels currently open, so a close can be checked.
///
/// A `console.time` label is global to the document and holds exactly one timer.
/// Asking the browser to close a label that is not open logs a warning and reports a
/// duration from whenever it was last open, which is how an inner timer sharing a
/// label with the one around it produces a plausible-looking wrong number rather
/// than an obvious zero. Tracking what is open turns that into a no-op.
///
/// This has to be a global lock rather than a `thread_local!`, because the thing
/// being tracked is global. Dioxus resumes an async task on whichever thread is free,
/// so two taxon resolutions in flight at once can sit on two different threads, and
/// a thread-local set is an empty set to each of them: both conclude the label is
/// free, both call `console.time_with_label`, and the browser reports the second as
/// `Timer "LOTUS:taxon_resolution" already exists.` That was the bug. A guard whose
/// scope is narrower than the resource it guards cannot guard it.
///
/// On wasm only, because that is the only target with a global label namespace:
/// native timings come from an [`Instant`] handle and cannot collide at all.
#[cfg(target_arch = "wasm32")]
static OPEN_TIMERS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> =
    // `LazyLock` because `HashSet::new` is not `const`: a `static` initialiser has to
    // be a constant expression, and building the set on first use is also what keeps
    // an empty set from costing an allocation for a page that opens no timers.
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

/// Insert `label`, reporting whether it was already open.
///
/// The lock is poisoned only if a thread panicked while holding it, which would
/// leave the set in a state no label is in. Recovering keeps the instrumentation
/// working rather than turning a bad measurement into a broken app.
#[cfg(target_arch = "wasm32")]
fn claim_timer(label: &str) -> bool {
    let mut open = OPEN_TIMERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    open.insert(label.to_owned())
}

/// Remove `label`, reporting whether it was open.
#[cfg(target_arch = "wasm32")]
fn release_timer(label: &str) -> bool {
    let mut open = OPEN_TIMERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    open.remove(label)
}

/// Start a `console.time()` block on WASM, or return the current timestamp on native.
#[cfg(target_arch = "wasm32")]
pub fn start_timer(label: &str) -> TimerHandle {
    if !claim_timer(label) {
        // Already open, which means this label is used from two places at once.
        // Say so once, in the app's own log rather than the browser console's, and
        // carry on: the inner timer cannot be made correct here, but the outer one
        // is still worth having.
        log::warn!(
            "event=perf state=reopened_timer label={label} detail=\"a console.time \
             label holds one timer; two sites sharing a label report each other's \
             duration\""
        );
        return wasm_now_ms();
    }
    web_sys::console::time_with_label(label);
    wasm_now_ms()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn start_timer(_label: &str) -> TimerHandle {
    Instant::now()
}

/// End a `console.time()` block on WASM and compute elapsed duration on native.
///
/// Closing a label that is not open is a no-op for the console and still returns a
/// duration measured from `started`, so an error path that closes early does not
/// make the caller's own measurement wrong.
#[cfg(target_arch = "wasm32")]
pub fn end_timer(label: &str, started: TimerHandle) -> Duration {
    if release_timer(label) {
        web_sys::console::time_end_with_label(label);
    }
    let elapsed_ms = (wasm_now_ms() - started).max(0.0);
    Duration::from_secs_f64(elapsed_ms / 1000.0)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn end_timer(_label: &str, started: TimerHandle) -> Duration {
    started.elapsed()
}

/// A timer that closes itself if it is dropped without [`Timer::end`].
///
/// A `console.time` label that is opened and never closed is worse than one that was
/// never opened. It does not merely lose its own measurement: the label stays taken
/// for the rest of the session, so every later `start_timer` with that label takes
/// the already-open branch, and the one duration that does get reported is measured
/// from the original open and grows with every subsequent call. On wasm that also
/// leaves a timer running in the console for the life of the page.
///
/// `resolve` had exactly this: the label was closed on its cache-hit return and not
/// on the SPARQL path, which is the path most searches take. The fix is not to
/// remember to close it on each of the returns that exist today but to close it on
/// the way out of the scope, because the next return someone adds would otherwise
/// leak it again.
#[must_use = "a Timer that is dropped without reading its duration measures nothing"]
pub struct Timer {
    label: String,
    handle: Option<TimerHandle>,
}

impl Timer {
    /// Open a timer that will close itself unless [`Timer::end`] is called first.
    pub fn start(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            handle: Some(start_timer(label)),
        }
    }

    /// Close the timer and return the elapsed duration.
    ///
    /// Consumes the `Timer`, so the duration cannot be read twice and [`Drop`] has
    /// nothing left to close.
    pub fn end(mut self) -> Duration {
        // `None` is unreachable: `end` owns `self`, so `Drop` has not run yet and
        // cannot have taken the handle. Zero rather than a panic, so a bug here
        // stays a wrong measurement instead of becoming a failed search.
        self.handle
            .take()
            .map_or(Duration::ZERO, |started| end_timer(&self.label, started))
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(started) = self.handle.take() {
            end_timer(&self.label, started);
        }
    }
}

#[cfg(test)]
mod tests {
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
    /// also skipping `perf.rs` below.
    fn label_on<'a>(line: &'a str, call: &str) -> Option<&'a str> {
        let rest = line.split(call).nth(1)?.trim_start();
        rest.strip_prefix('"')?.split('"').next()
    }

    /// Every `.rs` file under `src/` except this one, so a timer added anywhere is
    /// covered without the scanner reading its own pattern-matching code.
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
        found.retain(|path| path.file_name().is_none_or(|name| name != "perf.rs"));
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
    #[cfg(target_arch = "wasm32")]
    fn label_is_open(label: &str) -> Option<bool> {
        Some(!release_timer(label))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn label_is_open(_label: &str) -> Option<bool> {
        None
    }
}
