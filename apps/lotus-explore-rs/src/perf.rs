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
#[path = "perf/tests.rs"]
mod tests;
