// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Getting the structure editor to load in a desktop window.
//!
//! The editor is a single-page application, and a single-page application needs
//! a real path. Getting one inside a `dioxus-desktop` window is the whole problem,
//! and the ways of doing it are almost all closed:
//!
//! ## The frame cannot be pointed at a bundled path
//!
//! A frame load is a *navigation*, and `dioxus-desktop` allows its own scheme
//! exactly once and then denies it forever (`webview.rs:370`):
//!
//! ```text
//! let page_loaded = page_loaded.swap(true, Ordering::SeqCst);
//! return !page_loaded;
//! ```
//!
//! `wry` does not confine that handler to the main frame -- `navigation_policy`
//! is `decidePolicyForNavigationAction` and passes the *request* URL through
//! unfiltered -- so once the app itself has loaded, every further `dioxus://`
//! navigation is cancelled, including ours.
//!
//! ## and it cannot be pointed at a loopback port either
//!
//! That looks like the obvious answer, and it is the one that cannot work. The
//! same handler, a few lines further down:
//!
//! ```text
//! // External links always open somewhere else. Prevents the webview from navigating
//! if var.starts_with("http://") || var.starts_with("https://") || var.starts_with("mailto:") {
//!     _ = webbrowser::open(&var);
//!     return false;
//! }
//! ```
//!
//! There is no frame check, because `wry` does not provide one. So *any* `http://`
//! URL, including one on `127.0.0.1` that this process is serving, is handed to the
//! user's browser and cancelled in the window. A loopback server for the editor was
//! built and thrown away on this evidence: it works perfectly and cannot be pointed
//! at.
//!
//! `file://` is refused harder -- "Not allowed to load local resource", because the
//! page's origin is a custom scheme and `WebKit` will not load a local file from it.
//!
//! ## `blob:` gets the code running and loses the assets
//!
//! A `blob:` URL matches none of the handler's branches, so the frame loads. A blob
//! document has an **opaque origin**, though, so every one of the editor's own chunk
//! requests is cross-origin from a page that has the assets right there:
//! `main.e47c48ad.js` and `main.96b87be0.css` both 404 and the editor is a blank box.
//!
//! ## `srcdoc` is the one that works, with one problem left
//!
//! An `srcdoc` frame **inherits the embedding document's origin**, so the editor's
//! relative chunk references resolve against the app's own base and are served by
//! the bundler -- no second origin, no second server, no blob to keep alive. The
//! editor's code does run; it is the only option where that is true.
//!
//! What is left is the router. An `srcdoc` document's URL is `about:srcdoc`, and its
//! `pathname` is `srcdoc` -- with no leading slash. The editor is a React Router
//! application, and it does not match a path in that shape:
//!
//! ```text
//! [Warning] No routes matched location "srcdoc"
//! ```
//!
//! The document is same-origin with the app, which means its history is writable, so
//! [`PROBE`] gives it a real path before the editor's bundle runs. A `<base>` is
//! injected as well, so the editor's `./static/...` references resolve against the
//! app's root whatever the document's own base turns out to be.

/// The script injected into the editor's document, before its own bundle.
///
/// It does two things, and reports what happened to each so a failure is visible
/// rather than being another blank box.
pub const PROBE: &str = r#"
(function () {
  function report(fields) {
    try {
      parent.postMessage(Object.assign({ lotus: "ketcher_probe" }, fields), "*");
    } catch (error) { /* the frame is going away; nothing useful to do */ }
  }

  var gaveItAPath = false;
  var pathError = "";
  try {
    // An `about:blank` frame inherits this window's origin, and an `about:`
    // document has no path of its own -- its pathname is `blank`, which is not a
    // path either. Pushing the inherited origin's own root in is permitted here
    // and refused in an `srcdoc` frame ("protocols must match"), which is the
    // difference between the two and the reason for using this one.
    history.replaceState(null, "", "/");
    gaveItAPath = window.location.pathname.charAt(0) === "/";
    if (!gaveItAPath) {
      pathError = "replaceState did not take: " + window.location.pathname;
    }
  } catch (error) {
    pathError = String((error && error.message) || error);
  }

  report({ state: "prepared", path: window.location.pathname, gaveItAPath: gaveItAPath, pathError: pathError });

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
    report({ state: "document_loaded", path: window.location.pathname });
    // Ketcher is a React application that mounts into `#root`. If that container is
    // still empty a moment after load, its chunks did not arrive or its router
    // matched nothing -- and the window is an empty box either way.
    window.setTimeout(function () {
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

/// The editor's entry document, to be written into an empty frame.
///
/// # Errors
/// Returns a message if the entry document cannot be fetched, which in practice
/// means the editor is not in this build.
#[allow(clippy::future_not_send)] // a `WebView` round trip; see the module docs
pub async fn ketcher_frame_document(entry: &str) -> Result<String, String> {
    let entry_literal = serde_json::to_string(entry.trim())
        .map_err(|e| format!("could not encode the editor path: {e}"))?;
    // Derived from the entry path so the two cannot disagree: drop the last segment
    // and the editor's own `./...` references resolve as they do in a browser.
    let base = format!(
        "{}/",
        entry
            .trim_end_matches('/')
            .rsplit_once('/')
            .map_or("", |(directory, _)| directory)
    );
    let base_literal =
        serde_json::to_string(&base).map_err(|e| format!("could not encode the base: {e}"))?;
    let probe_literal =
        serde_json::to_string(PROBE).map_err(|e| format!("could not encode the probe: {e}"))?;

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
                // A script element, not bare text: text in a head is body content, so
                // it would render as words on the page and never run. And ahead of the
                // editor's own `defer`red bundle, because the router reads the path
                // while that bundle evaluates.
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

/// Put a document into an empty frame, from the window that owns it.
///
/// `document.write` rather than `srcdoc` or a navigation, because this is the only
/// one of the three that both reaches the `WebView` and leaves the document with a
/// URL the editor's router will match -- see the module docs for why each of the
/// others cannot work.
///
/// # Errors
/// Returns a message if the frame is not reachable, which means it is on another
/// origin and the document was not written.
#[allow(clippy::future_not_send)] // a `WebView` round trip
pub async fn write_into_frame(frame_id: &str, html: &str) -> Result<(), String> {
    let frame_literal = serde_json::to_string(frame_id)
        .map_err(|e| format!("could not encode the frame name: {e}"))?;
    let html_literal =
        serde_json::to_string(html).map_err(|e| format!("could not encode the document: {e}"))?;

    let script = format!(
        r#"(() => {{
            const frame = document.getElementById({frame_literal});
            if (!frame) {{
                return String("the editor's frame is not in the window");
            }}
            const doc = frame.contentDocument;
            if (!doc) {{
                return String("the editor's frame is on another origin, so it cannot be written to");
            }}
            doc.open();
            doc.write({html_literal});
            doc.close();
            return "";
        }})()"#
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

    /// The editor's real entry document, in the shape that matters: its bundle is
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

    /// The probe has to run *before* the editor's own bundle: the router reads the
    /// path while that bundle is evaluating, so a probe injected after it has already
    /// been too late.
    #[test]
    fn the_probe_is_a_script_ahead_of_the_editor_bundle() {
        let base = "/assets/ketcher/";
        let head = format!("<head><base href=\"{base}\"><script>{PROBE}</script>");
        let patched = ENTRY.replacen("<head>", &head, 1);

        // A script element, not bare text: text in a head is body content, so it
        // would render as words on the page and never run.
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
            "the probe must be ahead of the bundle, or the router has already read the path"
        );
        assert!(
            patched.find("<base").is_some_and(|at| at < probe_at),
            "and the base ahead of the probe"
        );
        // The editor's own markup survives, or there is nothing to mount into.
        assert!(patched.contains("id=\"root\""));
    }

    /// `replaceState` is the whole fix for the router, so it has to happen before
    /// the document reports anything -- otherwise the first thing known about it is
    /// that the path was still wrong.
    #[test]
    fn the_probe_gives_the_document_a_path_before_it_reports() {
        let set = PROBE.find("replaceState").expect("the probe sets a path");
        let reported = PROBE.find("state: \"prepared\"").expect("the first report");
        assert!(set < reported, "the path is set before the first report");
    }

    /// Whether `replaceState` actually takes is the engine's decision, and the
    /// probe reports it either way -- but a probe that forgot to check would leave
    /// the failure invisible.
    #[test]
    fn the_probe_checks_whether_the_path_took() {
        assert!(
            PROBE.contains("gaveItAPath"),
            "the outcome of replaceState is reported, not assumed"
        );
        assert!(
            PROBE.contains("window.location.pathname.charAt(0)"),
            "and it is checked by looking at the path, which is the thing the router reads"
        );
    }
}
