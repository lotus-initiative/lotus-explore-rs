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
/// A `console.time` label is global and holds exactly one timer. Asking the browser
/// to close a label that is not open logs a warning and reports a duration from
/// whenever it was last open, which is how an inner timer sharing a label with the
/// one around it produces a plausible-looking wrong number rather than an obvious
/// zero. Tracking what is open turns that into a no-op.
///
/// On wasm only, because that is the only target with a global label namespace:
/// native timings come from an [`Instant`] handle and cannot collide at all.
#[cfg(target_arch = "wasm32")]
thread_local! {
    /// The `thread_local!` macro rather than the `#[thread_local]` attribute, which
    /// is still unstable for statics with destructors.
    ///
    /// Owns its keys because not every label is a literal: the download timers are
    /// built from the format, and those pass a `&str` with no `'static` to keep.
    static OPEN_TIMERS: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}

/// Start a `console.time()` block on WASM, or return the current timestamp on native.
#[cfg(target_arch = "wasm32")]
pub fn start_timer(label: &str) -> TimerHandle {
    // `contains` before `insert` so the common case -- a label that is not already
    // open -- does not allocate for a key that is about to be stored anyway.
    let already_open = OPEN_TIMERS.with(|open| open.borrow().contains(label));
    if already_open || !OPEN_TIMERS.with(|open| open.borrow_mut().insert(label.to_owned())) {
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
    if OPEN_TIMERS.with(|open| open.borrow_mut().remove(label)) {
        web_sys::console::time_end_with_label(label);
    }
    let elapsed_ms = (wasm_now_ms() - started).max(0.0);
    Duration::from_secs_f64(elapsed_ms / 1000.0)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn end_timer(_label: &str, started: TimerHandle) -> Duration {
    started.elapsed()
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

    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

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
}
