// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

// A `WebView` round trip via `document::eval`, so the future is not `Send`.
#![expect(
    clippy::future_not_send,
    reason = "`document::eval` holds a WebView channel handle that is not `Send`"
)]

use dioxus::prelude::*;
use serde_json::Value;

fn parse_quickstatements_text(text: &str) -> Option<Vec<String>> {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    (!lines.is_empty()).then_some(lines)
}

/// Ask the `Citation.js` bridge in the page to format a `DOI` as
/// `QuickStatements`, using `document::eval` so both browser and desktop
/// `WebView` run the same path.
pub(super) async fn fetch_reference_quickstatements(doi: &str) -> Option<Vec<String>> {
    let doi_literal = serde_json::to_string(doi.trim()).ok()?;
    let script = format!(
        r"(async () => {{
            try {{
                const bridge = window.__lotusCitation;
                if (!bridge || typeof bridge.quickStatements !== 'function') {{
                    dioxus.send({{ lotus_citation_value: null }});
                    return;
                }}
                const value = await bridge.quickStatements({doi_literal});
                dioxus.send({{ lotus_citation_value: typeof value === 'string' ? value : null }});
            }} catch (_error) {{
                dioxus.send({{ lotus_citation_value: null }});
            }}
        }})();"
    );

    let mut eval = document::eval(&script);
    let outcome: Value = eval.recv().await.ok()?;
    let text = outcome.get("lotus_citation_value")?.as_str()?;
    parse_quickstatements_text(text)
}

#[cfg(test)]
#[path = "reference_metadata/tests.rs"]
mod tests;
