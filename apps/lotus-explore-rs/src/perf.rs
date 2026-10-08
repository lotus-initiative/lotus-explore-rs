// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Performance monitoring and logging infrastructure for SPARQL query execution.
//!
//! The platform edge is [`clock`]. Everything here — the registry of open labels,
//! the duplicate-open policy, the drop guard — is the same code on both targets, so
//! the same tests pin it on both.

mod clock;

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Duration;

/// What an open timer holds. Target-dependent by construction; see [`clock`].
pub type TimerHandle = clock::WasmHandle;

/// The `console.time()` labels currently open, so a close can be checked.
///
/// A `console.time` label is global to the document and holds exactly one timer. Asking
/// the browser to close a label that is not open logs a warning and reports a duration
/// from whenever it was last open, which is how an inner timer sharing a label with the
/// one around it produces a plausible-looking wrong number rather than an obvious zero.
/// Tracking what is open turns that into a no-op.
///
/// This has to be a global lock rather than a `thread_local!`, because the thing being
/// tracked is global. Dioxus resumes an async task on whichever thread is free, so two
/// taxon resolutions in flight at once can sit on two different threads, and a
/// thread-local set is an empty set to each of them: both conclude the label is free,
/// both open it, and the browser reports the second as
/// `Timer "LOTUS:taxon_resolution" already exists.` That was the bug.
///
/// Native has no global label namespace and cannot collide, but it tracks the same
/// registry anyway: the duplicate-open case is then reportable and testable on the
/// target CI actually runs, rather than only on the one it cannot execute.
///
/// `LazyLock` because `HashSet::new` is not `const`: a `static` initialiser has to be a
/// constant expression, and building the set on first use also keeps an empty set from
/// costing an allocation for a page that opens no timers.
static OPEN_LABELS: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Insert `label`, reporting whether it was already open.
///
/// The lock is poisoned only if a thread panicked while holding it, which would leave
/// the set in a state no label is in. Recovering keeps the instrumentation working
/// rather than turning a bad measurement into a broken app.
fn claim_timer(label: &str) -> bool {
    let mut open = OPEN_LABELS.lock().unwrap_or_else(PoisonError::into_inner);
    open.insert(label.to_owned())
}

/// Remove `label`, reporting whether it was open.
fn release_timer(label: &str) -> bool {
    let mut open = OPEN_LABELS.lock().unwrap_or_else(PoisonError::into_inner);
    open.remove(label)
}

/// Whether `label` is currently held.
///
/// Test-facing: the duplicate-open policy is observed through the log line and through
/// what the browser does, neither of which a unit test can read. Both targets share the
/// registry, so this answers the same question in both.
#[cfg(test)]
fn label_is_open(label: &str) -> bool {
    let open = OPEN_LABELS.lock().unwrap_or_else(PoisonError::into_inner);
    open.contains(label)
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

    clock::log_line(&msg);
}

/// Start a `console.time()` block on WASM, or start an [`std::time::Instant`] on native.
#[must_use]
pub fn start_timer(label: &str) -> clock::WasmHandle {
    if !claim_timer(label) {
        // Already open, which means this label is used from two places at once. Say so
        // in the app's own log rather than the browser console's, and carry on: the
        // inner timer cannot be made correct here, but the outer one is still worth
        // having, and on native it is now the same diagnostic rather than nothing.
        log::warn!(
            "event=perf state=reopened_timer label={label} detail=\"a console.time \
             label holds one timer; two sites sharing a label report each other's \
             duration\""
        );
    }
    clock::open(label);
    clock::start()
}

/// End a `console.time()` block on WASM and compute elapsed duration on native.
///
/// Closing a label that is not open is a no-op for the console and still returns a
/// duration measured from `started`, so an error path that closes early does not make
/// the caller's own measurement wrong.
pub fn end_timer(label: &str, started: clock::WasmHandle) -> Duration {
    if release_timer(label) {
        clock::close(label);
    }
    clock::elapsed(started)
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
    handle: Option<clock::WasmHandle>,
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
#[path = "perf/tests.rs"]
mod tests;
