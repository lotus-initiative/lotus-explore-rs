// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! How the curation page reaches the `RDKit` bridge in the page.
//!
//! `document::eval` is the only channel that works in every renderer. The
//! alternative -- `web_sys::window()` and a reflected call into the bridge -- is
//! wasm-only, because a native build has no `window` object even though the
//! window *is* a `WebView`. A native build that used it therefore had to bypass
//! `RDKit` and call a third-party HTTP service instead, which is why a desktop
//! window behaved differently from a browser.
//!
//! One shape of call works on both, and it is not the obvious one. `eval`
//! returns what the script evaluates to *synchronously*: a returned promise
//! arrives as `null`, not as its resolved value. So the bridge exposes a
//! synchronous accessor, and readiness is a separate call that is polled. A
//! single `await bridge.convert(...)` silently produced `null` and a parse
//! error, which is what this module exists to prevent.

// A `WebView` round trip, so these futures are not `Send`. Every caller is on a
// component's own single-threaded task, which is the right shape for it.
#![allow(clippy::future_not_send)]

use dioxus::prelude::*;
use serde_json::Value;

use super::CurationError;

/// How long to wait for `RDKit`'s 7 MB wasm module to compile, in milliseconds.
///
/// Generous, because the first load compiles the module and a cold start on a
/// slow disk is not the same as a bad build. The wait is a poll loop, so a slow
/// load costs nothing but a few `eval` round trips.
const READY_TIMEOUT_MS: u64 = 60_000;
const READY_POLL_MS: u64 = 100;

/// Call a method on the `RDKit` bridge and return its answer.
///
/// Waits for `RDKit` to finish loading first. The wait is bounded, and a load
/// that failed reports the error the loader recorded rather than timing out.
///
/// # Errors
/// Returns a message if the bridge is missing, the load failed, the wait timed
/// out, or the call itself threw.
pub(super) async fn rdkit_bridge_call(method: &str, smiles: &str) -> Result<Value, CurationError> {
    wait_until_ready().await?;

    // The SMILES is user input, so it goes in as a JSON string literal rather
    // than being interpolated raw: a quote or a backslash would otherwise end
    // the literal and change what the script parses as.
    let smiles_literal = serde_json::to_string(smiles.trim())
        .map_err(|e| CurationError::Parse(format!("could not encode the structure: {e}")))?;
    let method_literal = serde_json::to_string(method)
        .map_err(|e| CurationError::Parse(format!("could not encode the method: {e}")))?;

    eval(&format!(
        r"window.__lotusRdkit[{method_literal}]({smiles_literal})"
    ))
    .await
}

/// Start the load and poll until the toolkit is usable.
///
/// # Errors
/// Returns a message if the bridge is absent, the load recorded an error, or the
/// wait ran out.
async fn wait_until_ready() -> Result<(), CurationError> {
    // Kick the load off. The bridge is single-flight, so this is safe to call on
    // every conversion.
    eval("window.__lotusRdkit.start()").await?;

    let mut waited = 0_u64;
    loop {
        let state =
            eval(r"({ ready: window.__lotusRdkit.isReady(), error: window.__lotusRdkit.error() })")
                .await?;

        if state.get("ready").and_then(Value::as_bool) == Some(true) {
            return Ok(());
        }
        if let Some(error) = state.get("error").and_then(Value::as_str) {
            return Err(CurationError::Http(format!(
                "rdkit.js failed to load: {error}"
            )));
        }
        if waited >= READY_TIMEOUT_MS {
            return Err(CurationError::Http(format!(
                "rdkit.js was still loading after {READY_TIMEOUT_MS}ms"
            )));
        }
        wait(READY_POLL_MS).await;
        waited += READY_POLL_MS;
    }
}

/// Evaluate `script` and return its value.
///
/// # Errors
/// Returns a message if the script throws or its value cannot be read.
async fn eval(script: &str) -> Result<Value, CurationError> {
    document::eval(script)
        .await
        .map_err(|e| CurationError::Http(format!("evaluating the RDKit bridge failed: {e}")))
}

/// Wait `ms` milliseconds.
///
/// The poll interval is a tenth of a second and the wait can run for a minute
/// while a seven megabyte wasm module compiles, so this is a real timer rather
/// than a busy loop. `futures-timer` is the one timer here that builds for both
/// renderers without dragging a runtime in with it: `tokio` is native-only and
/// not even enabled for the desktop build, `gloo-timers` is wasm-only, and
/// sleeping on a spawned thread would wake the runtime for a no-op.
///
/// # Errors
/// Never.
async fn wait(ms: u64) {
    futures_timer::Delay::new(std::time::Duration::from_millis(ms)).await;
}

#[cfg(test)]
mod tests {
    // A test that fails to encode is reporting, not panicking.
    #![allow(clippy::expect_used)]

    /// The SMILES is user input and goes into a JS string literal. The property
    /// that matters is that the literal parses back to exactly the input, so
    /// nothing inside it can terminate the literal early and change what the
    /// script runs.
    #[test]
    fn a_structure_round_trips_through_its_string_literal() {
        for input in [
            "CCO",
            r#"C/C=C/C""#,      // a double quote
            r"C\\C",            // a backslash
            "'); alert(1); ('", // an attempt to close the literal
            "C[C@@H](C(=O)O)N", // stereochemistry
            "line\\nbreak",     // a newline
        ] {
            let literal = serde_json::to_string(input).expect("encodes");
            let back: String = serde_json::from_str(&literal).expect("the literal parses back");
            assert_eq!(back, input, "{literal} did not round-trip");
            assert!(literal.starts_with('"'), "{literal} is not double-quoted");
        }
    }
}
