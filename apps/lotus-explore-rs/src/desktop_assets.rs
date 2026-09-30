// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Getting the structure editor to load in a desktop window.
//!
//! The editor is a single-page application, and a single-page application needs a
//! real origin *and* a real path. `dioxus-desktop` allows neither by accident, so
//! every option is worth writing down. All of this is read out of the navigation
//! handler in `dioxus-desktop/src/webview.rs:369`, which `wry` calls with the
//! *request* URL and no distinction between the main frame and a subframe:
//!
//! ```text
//! if var.starts_with("dioxus://") || ... {
//!     let page_loaded = page_loaded.swap(true, SeqCst);
//!     return !page_loaded;                       // allowed once, then denied
//! }
//! if var.starts_with("http://") || var.starts_with("https://") || var.starts_with("mailto:") {
//!     _ = webbrowser::open(&var);                 // your browser, not this window
//!     return false;
//! }
//! navigation_handler.map(|f| f(&var)).unwrap_or(true)   // everything else: allowed
//! ```
//!
//! - **A bundled `dioxus://` path** is the first branch. The app's own load takes
//!   the single permitted navigation, so a frame pointed at the bundle is denied
//!   before a document exists -- which is why neither `load` nor `error` fires.
//! - **A loopback port** is the second branch, and is worse than refused. The
//!   editor opens in the user's browser and the frame is cancelled. A static
//!   server for the editor was written and thrown away on this evidence: it worked
//!   perfectly and could not be pointed at.
//! - **`file://`** is refused by `WebKit` outright: "Not allowed to load local
//!   resource", because the page's origin is a custom scheme.
//! - **`blob:`** matches no branch, so the frame loads, and a blob document has an
//!   **opaque origin**. Every one of the editor's own chunk requests is then
//!   cross-origin from a page that has the assets right there: `main.e47c48ad.js`
//!   and `main.96b87be0.css` both 404.
//! - **`srcdoc`** matches no branch and **inherits this document's origin**, so the
//!   chunks do load. But the document's URL is `about:srcdoc` and its pathname is
//!   `srcdoc`, which the editor's router will not match: `No routes matched
//!   location "srcdoc"`. And it cannot be corrected, because an `about:srcdoc`
//!   document may not adopt a `dioxus://` URL -- `WebKit` refuses the
//!   `history.replaceState` outright, on the grounds that *"Protocols, domains,
//!   ports, usernames, and passwords must match."*
//!
//! ## `about:blank` is what is left, and it is enough
//!
//! It matches no branch either, so it is allowed -- and an `about:blank` frame
//! **inherits the embedding document's origin**. So both halves work:
//!
//! - The editor's relative chunk references resolve against the app's own base and
//!   are served by the bundler, from the same origin, with no second server and no
//!   blob to keep alive.
//! - `history.replaceState(null, "", "/")` resolves against that inherited origin
//!   and is permitted, so the router is given a pathname with a leading slash and
//!   its catch-all route matches.
//!
//! So: an empty frame, the document written into it from here, and one line of
//! script ahead of the editor's own bundle. [`PROBE`] reports whether each part
//! worked, because a frame whose every chunk 404s looks exactly like one that
//! loaded, and that is how the two previous attempts went unnoticed.

/// The frame's id, shared between the markup and the code that fills it.
pub const FRAME_ID: &str = "lotus-ketcher-frame";

/// The script injected into the editor's document, ahead of its own bundle.
///
/// It has to run first: the editor's router reads the path while its bundle is
/// evaluating, so a probe injected after it has already been too late.
pub const PROBE: &str = r#"
(function () {
  function report(fields) {
    try {
      parent.postMessage(Object.assign({ lotus: "ketcher_probe" }, fields), "*");
    } catch (error) { /* the frame is going away; nothing useful to do */ }
  }

  // The one line that makes this work. An `about:blank` document has no path of
  // its own -- its pathname is `blank` -- and the editor is a React Router
  // application that will not match that. Pushing the inherited origin's own root
  // in is permitted here and refused from an `srcdoc` frame, and that difference
  // is the entire reason for using this mechanism.
  var took = false;
  var why = "";
  try {
    history.replaceState(null, "", "/");
    took = window.location.pathname.charAt(0) === "/";
    if (!took) {
      why = "replaceState did not take: " + window.location.pathname;
    }
  } catch (error) {
    why = String((error && error.message) || error);
  }
  report({ state: "prepared", path: window.location.pathname, took: took, why: why });

  window.addEventListener("error", function (event) {
    report({
      state: "error",
      message: String((event && event.message) || event),
      source: String((event && event.filename) || ""),
      line: (event && event.lineno) || 0
    });
  });
  window.addEventListener("unhandledrejection", function (event) {
    var reason = event && event.reason;
    report({ state: "rejection", message: String((reason && reason.message) || reason) });
  });
  window.addEventListener("load", function () {
    window.setTimeout(function () {
      // Ketcher is a React application that mounts into `#root`. If that is still
      // empty a moment after load then its chunks arrived and its router matched
      // nothing, or neither -- and the window is an empty box either way.
      var root = document.getElementById("root");
      report({
        state: "probed",
        mounted: Boolean(root && root.childElementCount > 0),
        children: root ? root.childElementCount : -1,
        scripts: document.querySelectorAll("script[src]").length,
        path: window.location.pathname
      });
    }, 3000);
  });
})();
"#;

/// The editor's entry document, ready to be written into the frame.
///
/// # Errors
/// Returns a message if the entry document cannot be fetched, which in practice
/// The directory an entry document sits in, with a trailing slash.
///
/// Derived from the entry path so the `<base href>` and the entry cannot disagree:
/// dropping the last segment is what makes the editor's own `./...` references
/// resolve as they do in a browser.
fn entry_directory(entry: &str) -> String {
    format!(
        "{}/",
        entry
            .trim_end_matches('/')
            .rsplit_once('/')
            .map_or("", |(directory, _)| directory)
    )
}

/// means the editor is not in this build.
#[allow(clippy::future_not_send)] // a `WebView` round trip; see the module docs
pub async fn ketcher_document(entry: &str) -> Result<String, String> {
    let entry_literal =
        serde_json::to_string(entry.trim()).map_err(|e| format!("could not encode: {e}"))?;
    let base = entry_directory(entry);
    let base_literal =
        serde_json::to_string(&base).map_err(|e| format!("could not encode: {e}"))?;
    let probe_literal =
        serde_json::to_string(PROBE).map_err(|e| format!("could not encode: {e}"))?;

    let script = format!(
        r#"(async () => {{
            const fail = (message) => dioxus.send({{ ketcher_error: String(message) }});
            try {{
                const response = await fetch({entry_literal});
                if (!response.ok) {{
                    fail("the structure editor is not in this build (HTTP " + response.status + ")");
                    return;
                }}
                const html = await response.text();
                const probe = {probe_literal};
                const base = {base_literal};
                // A script *element*, not bare text: text in a head is body content,
                // so it would render as words on the page and never run. And ahead of
                // the editor's own `defer`red bundle, which is the whole ordering
                // requirement.
                const head = '<head><base href="' + base + '"><script>' + probe + '</script>';
                dioxus.send({{
                    ketcher_document: /<base\s/i.test(html) ? html : html.replace(/<head>/i, head)
                }});
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
        .get("ketcher_document")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "the structure editor could not be prepared".to_string())
}

/// Put a document into the empty frame, from the window that owns it.
///
/// `document.write` rather than a navigation, because this is the only one of the
/// three that both reaches the `WebView` and leaves the document with a URL the
/// editor's router will match.
///
/// # Errors
/// Returns a message if the frame is not reachable, which means it did not inherit
/// this document's origin and so cannot be written to.
#[allow(clippy::future_not_send)] // a `WebView` round trip
pub async fn write_into_frame(html: &str) -> Result<(), String> {
    let id = serde_json::to_string(FRAME_ID).map_err(|e| format!("could not encode: {e}"))?;
    let html_literal = serde_json::to_string(html).map_err(|e| format!("could not encode: {e}"))?;

    // `dioxus.send`, not a returned value. On this target an eval's return value
    // is not delivered: `recv` waits for a `send`, and a script that only returns
    // leaves it waiting until the eval is reported as finished -- which looks
    // exactly like a failure, even though the document was written.
    let script = format!(
        r#"(async () => {{
            const frame = document.getElementById({id});
            if (!frame) {{
                dioxus.send("the editor's frame is not in the window");
                return;
            }}
            const doc = frame.contentDocument;
            if (!doc) {{
                dioxus.send("the editor's frame did not inherit this origin, so it cannot be written to");
                return;
            }}
            doc.open();
            doc.write({html_literal});
            doc.close();
            dioxus.send("");
        }})();"#
    );

    let mut eval = dioxus::prelude::document::eval(&script);
    let outcome: serde_json::Value = eval
        .recv()
        .await
        .map_err(|e| format!("the structure editor could not be placed: {e}"))?;

    match outcome.as_str() {
        None | Some("") => Ok(()),
        Some(reason) => Err(reason.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    // A test that fails on a bad fixture is reporting, not panicking on input.
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

    use super::PROBE;

    /// Ketcher's real entry document, in the two shapes that matter: its bundle is
    /// a `defer`red script in the head, and it mounts into `#root`.
    const ENTRY: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"/>\
        <link rel=\"icon\" href=\"./favicon.ico\"/><title>Ketcher</title>\
        <script defer=\"defer\" src=\"./static/js/main.e47c48ad.js\"></script>\
        <link href=\"./static/css/main.96b87be0.css\" rel=\"stylesheet\"></head>\
        <body><div id=\"root\"></div></body></html>";

    /// The base is derived from the entry path, so the two cannot drift.
    #[test]
    fn a_base_is_the_directory_of_the_entry_document() {
        let base = |entry: &str| {
            format!(
                "{}/",
                entry
                    .trim_end_matches('/')
                    .rsplit_once('/')
                    .map_or("", |(directory, _)| directory)
            )
        };
        assert_eq!(base("/assets/ketcher/index.html"), "/assets/ketcher/");
        assert_eq!(base("/assets/ketcher/"), "/assets/");
        assert_eq!(base("index.html"), "/");
    }

    /// The probe has to run *before* the editor's own bundle, and has to be a
    /// script element. Both are load-bearing: a probe that runs late reads the path
    /// too late to influence anything, and a probe inserted as text renders as
    /// words on the page.
    #[test]
    fn the_probe_is_a_script_ahead_of_the_editor_bundle() {
        let head = format!("<head><base href=\"/assets/ketcher/\"><script>{PROBE}</script>");
        let patched = ENTRY.replacen("<head>", &head, 1);

        assert!(
            patched.contains("<script>"),
            "a script element, not bare text"
        );
        assert!(patched.contains("ketcher_probe"), "and it is the probe");

        let probe_at = patched.find("<script>").expect("the probe");
        let bundle_at = patched
            .find("main.e47c48ad.js")
            .expect("the editor's bundle");
        assert!(
            probe_at < bundle_at,
            "ahead of the bundle, or the router has already read the path"
        );
        assert!(
            patched.find("<base").is_some_and(|at| at < probe_at),
            "and the base ahead of the probe"
        );
        // The editor's own markup survives, or there is nothing to mount into.
        assert!(patched.contains("id=\"root\""));
    }

    /// `replaceState` is the whole fix for the router, so it happens before the
    /// document reports anything -- otherwise the first thing known about it is
    /// that the path was still wrong.
    #[test]
    fn the_probe_gives_the_document_a_path_before_it_reports() {
        let set = PROBE.find("replaceState").expect("the probe sets a path");
        let reported = PROBE.find("state: \"prepared\"").expect("the first report");
        assert!(set < reported, "the path is set before the first report");
    }

    /// Whether `replaceState` takes is the engine's decision and the probe reports
    /// it either way -- but a probe that forgot to check would leave the failure
    /// invisible, which is what the first two attempts did.
    #[test]
    fn the_probe_checks_whether_the_path_took() {
        assert!(
            PROBE.contains("window.location.pathname.charAt(0)"),
            "checked by looking at the path, which is the thing the router reads"
        );
    }

    /// And it reports whether the editor mounted, which is the only thing that
    /// distinguishes a working editor from a blank box.
    #[test]
    fn the_probe_reports_whether_the_editor_mounted() {
        assert!(
            PROBE.contains("mounted: Boolean(root && root.childElementCount > 0)"),
            "the mount is checked against the container Ketcher renders into"
        );
    }
}
