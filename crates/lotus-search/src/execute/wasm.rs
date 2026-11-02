// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The browser's retry wait. Only compiled for wasm32, so no native test can
//! observe it; `.cargo/mutants.toml` excludes this file for that reason.

use js_sys::Promise;
use std::time::Duration;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

/// The wasm counterpart, which yields instead of waiting.
///
/// Awaiting a resolved promise is the platform's own "resume on the next turn of
/// the event loop", and a retry loop driven from a single-threaded browser
/// runtime needs it: a chain of requests that resolve without ever handing
/// control back would block the page. It costs a turn of the loop instead of the
/// backoff, which is the right trade on a network failure.
///
/// Not the legacy timer, which was here first and **panicked in the browser**: on
/// `wasm32-unknown-unknown` it falls back to the unimplemented `std::time::Instant`, so the
/// failure read `time not implemented on this platform` from inside a retry -- reached only
/// once a request had already failed, exactly when nobody is watching the console. The
/// build was green and the app died on its first network error.
pub(super) async fn backoff(_duration: Duration) {
    // A client that cannot reach the endpoint will not reach it in a millisecond either, and
    // blocking the browser's only thread to find that out is worse than retrying at once.
    // A resolved promise is the platform's own "run this on the next turn of the event
    // loop", and awaiting one is how a future yields on a single-threaded browser runtime.
    // The `Result` cannot be awaited usefully: an already-resolved promise does not reject,
    // and there is nothing here for a failure to abort.
    //
    // Mutating *this* body survives a native run, and that is not a gap in the tests:
    // `cfg(target_arch = "wasm32")` keeps none of it in the test binary, so no mutation of it
    // is observable from one. The same edit to the `#[cfg(not(target_arch = "wasm32"))]`
    // twin in `execute.rs` *is* caught, by `a_retry_waits_before_it_happens`. Killing this one takes a
    // browser test, which is what `wasm-bindgen-test` is for.
    let _ = JsFuture::from(Promise::resolve(&JsValue::UNDEFINED)).await;
}
