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
//! Getting a value *out* of the page is the part that is easy to get wrong.
//! Awaiting the `Eval` itself returns what the script evaluates to
//! synchronously, so an `async` bridge method arrives as `null`. Dioxus's
//! supported answer is the eval channel: the script pushes the result with
//! `dioxus.send` once its promise settles, and Rust awaits `recv`. That is what
//! this module does, and it is why there is no polling loop here.

// A `WebView` round trip, so these futures are not `Send`. Every caller is on a
// component's own single-threaded task, which is the right shape for it.
// The search future is not `Send`: `on_phase` is a closure over a Dioxus
// `Signal`, which is a `RefCell`. See the full explanation in
// `features/explore/executor.rs`, which is the same cause reached from here.
#![expect(
    clippy::future_not_send,
    reason = "`on_phase` captures a Dioxus `Signal`, which is a `RefCell` and not `Sync`"
)]

use dioxus::prelude::*;
use serde_json::Value;

use super::CurationError;

/// Call a method on the `RDKit` bridge and return its answer.
///
/// Waits for `RDKit` to finish loading first, which the bridge does internally.
///
/// # Errors
/// Returns a message if the bridge is missing, the load failed, the call threw,
/// or nothing arrived on the channel.
pub(super) async fn rdkit_bridge_call(method: &str, smiles: &str) -> Result<Value, CurationError> {
    // The SMILES is user input, so it goes in as a JSON string literal rather
    // than being interpolated raw: a quote or a backslash would otherwise end
    // the literal and change what the script parses as.
    let smiles_literal = serde_json::to_string(smiles.trim())
        .map_err(|e| CurationError::Parse(format!("could not encode the structure: {e}")))?;

    // `method` is a literal at every call site, so it is not attacker-controlled
    // and needs no escaping. `dioxus.send` is called on both paths, so the
    // channel always produces exactly one value and `recv` cannot hang.
    let script = format!(
        r#"(async () => {{
            const fail = (message) => dioxus.send({{ lotus_rdkit_error: String(message) }});
            try {{
                const bridge = window.__lotusRdkit;
                if (!bridge) {{
                    fail("the RDKit bridge is not loaded");
                    return;
                }}
                const value = await bridge.{method}({smiles_literal});
                dioxus.send({{ lotus_rdkit_value: value === undefined ? null : value }});
            }} catch (error) {{
                fail((error && error.message) || error);
            }}
        }})();"#
    );

    let mut eval = document::eval(&script);
    let outcome: Value = eval
        .recv()
        .await
        .map_err(|e| CurationError::Http(format!("the RDKit bridge did not answer: {e}")))?;

    if let Some(message) = outcome.get("lotus_rdkit_error").and_then(Value::as_str) {
        return Err(CurationError::Http(format!(
            "rdkit.js {method} failed: {message}"
        )));
    }
    outcome
        .get("lotus_rdkit_value")
        .cloned()
        .ok_or_else(|| CurationError::Http(format!("rdkit.js {method} returned no value")))
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
