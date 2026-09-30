// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Getting the structure editor to load in a desktop window.
//!
//! The editor is an `<iframe>`, and a frame load is a *navigation*.
//! `dioxus-desktop` allows its own scheme exactly once and then denies it
//! forever (`webview.rs:370`):
//!
//! ```text
//! let page_loaded = page_loaded.swap(true, Ordering::SeqCst);
//! return !page_loaded;
//! ```
//!
//! `wry` does not confine that handler to the main frame: `navigation_policy` in
//! `wry-0.53.5/src/wkwebview/navigation.rs` is `decidePolicyForNavigationAction`
//! and passes the URL through unfiltered -- so once the app itself has loaded,
//! every further `dioxus://` navigation is cancelled, including ours. The frame is
//! refused before a document exists, which is why neither `load` nor `error`
//! fires and the panel is an empty box. `RDKit` is unaffected because a
//! `<script src>` is a subresource and never reaches the navigation handler.
//!
//! Two obvious workarounds do not survive contact with the `WebView`:
//!
//! - Pointing the frame at the bundled `/assets/ketcher/index.html` is the
//!   navigation that is being refused.
//! - Pointing it at a `file://` URL is refused harder: "Not allowed to load local
//!   resource", because the page's origin is a custom scheme and `WebKit` will not
//!   load a local file from it.
//!
//! A `blob:` URL is neither, and that is what this does. The entry document is
//! fetched as a subresource -- allowed, and the whole directory is already in the
//! bundle -- and handed to the frame as a blob. `blob:` matches none of the
//! navigation handler's branches, so it falls through to the permissive default,
//! and the blob inherits the creating origin, so the editor's relative chunk
//! references resolve to the same `/assets/ketcher/` subresources they would have
//! in a browser.
//!
//! A `<base>` is injected because a blob URL's own base is its opaque path, not
//! the directory it was created from.

/// The URL to give the editor's frame, on a native build.
///
/// # Errors
/// Returns a message if the entry document cannot be fetched or the blob cannot
/// be created, which in practice means the editor is not in this build.
#[allow(clippy::future_not_send)] // a `WebView` round trip; see the module docs
pub async fn ketcher_frame_url(entry: &str) -> Result<String, String> {
    let entry_literal = serde_json::to_string(entry.trim())
        .map_err(|e| format!("could not encode the editor path: {e}"))?;

    // The base is derived from the entry path so the two cannot disagree: drop
    // the last segment and the editor's own `./...` references resolve exactly as
    // they do in a browser.
    let base = format!(
        "{}/",
        entry
            .trim_end_matches('/')
            .rsplit_once('/')
            .map_or("", |(d, _)| d)
    );
    let base_literal = serde_json::to_string(&base)
        .map_err(|e| format!("could not encode the editor base: {e}"))?;

    let script = format!(
        r#"(async () => {{
            const fail = (message) => dioxus.send({{ ketcher_error: String(message) }});
            try {{
                const response = await fetch({entry_literal});
                if (!response.ok) {{
                    fail("the structure editor is not in this build (HTTP " + response.status + ")");
                    return;
                }}
                let html = await response.text();
                // A blob URL's base is its own opaque path, so without this the
                // editor's relative chunk references resolve against nothing.
                const base = {base_literal};
                const patched = /<base\s/i.test(html)
                    ? html
                    : html.replace(/<head>/i, '<head><base href="' + base + '">');
                const blob = new Blob([patched], {{ type: "text/html" }});
                dioxus.send({{ ketcher_url: URL.createObjectURL(blob) }});
            }} catch (error) {{
                fail((error && error.message) || error);
            }}
        }})();"#
    );

    let mut eval = dioxus::prelude::document::eval(&script);
    let outcome: serde_json::Value = eval
        .recv()
        .await
        .map_err(|e| format!("the structure editor could not be prepared: {e}"))?;

    if let Some(message) = outcome
        .get("ketcher_error")
        .and_then(serde_json::Value::as_str)
    {
        return Err(message.to_string());
    }
    outcome
        .get("ketcher_url")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "the structure editor could not be prepared".to_string())
}

#[cfg(test)]
mod tests {
    /// The injected `<base>` has to be the directory, not the file, and it is
    /// derived rather than hard-coded so the two cannot drift.
    #[test]
    fn a_base_is_the_directory_of_the_entry_document() {
        let base = |entry: &str| {
            format!(
                "{}/",
                entry
                    .trim_end_matches('/')
                    .rsplit_once('/')
                    .map_or("", |(d, _)| d)
            )
        };
        assert_eq!(base("/assets/ketcher/index.html"), "/assets/ketcher/");
        assert_eq!(base("/assets/ketcher/"), "/assets/");
    }
}