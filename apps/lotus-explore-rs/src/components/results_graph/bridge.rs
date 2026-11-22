// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Discrete `document::eval` bridge to `window.LotusGraphGL` (`lotus-graph-gl.js`).
//!
//! Only discrete commands (graph load, theme, mode, selection, zoom/reset/pause,
//! PNG snapshot) cross the Rust ↔ JS boundary — never per-frame animation ticks.

use dioxus::prelude::*;
use lotus_explore_core::graph::GraphPayload;

/// DOM id of the WebGL2 `<canvas>`.
pub const CANVAS_ID: &str = "lotus-network-canvas";

/// DOM id of the anti-colliding floating label overlay container.
pub const LABELS_ID: &str = "lotus-network-labels";

/// Discrete event emitted by `lotus-graph-gl.js` over `lotus-graph-event`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphEvent {
    ScriptLoaded,
    Ready,
    Unavailable,
    Select(Option<usize>),
    Hover(Option<usize>),
    Settled,
    ContextLost,
    ContextRestored,
    PngData(String),
}

/// Parse one `CustomEvent.detail` payload received via `dioxus.send`.
pub fn parse_graph_event(value: &serde_json::Value) -> Option<GraphEvent> {
    let kind = value.get("type").and_then(serde_json::Value::as_str)?;
    match kind {
        "script_loaded" => Some(GraphEvent::ScriptLoaded),
        "ready" => Some(GraphEvent::Ready),
        "unavailable" => Some(GraphEvent::Unavailable),
        "settled" => Some(GraphEvent::Settled),
        "contextlost" => Some(GraphEvent::ContextLost),
        "contextrestored" => Some(GraphEvent::ContextRestored),
        "select" => Some(GraphEvent::Select(parse_optional_index(value.get("index")))),
        "hover" => Some(GraphEvent::Hover(parse_optional_index(value.get("index")))),
        "png_data" => value
            .get("dataUrl")
            .and_then(serde_json::Value::as_str)
            .map(|url| GraphEvent::PngData(url.to_owned())),
        _ => None,
    }
}

fn parse_optional_index(value: Option<&serde_json::Value>) -> Option<usize> {
    let idx = value.and_then(serde_json::Value::as_u64)?;
    usize::try_from(idx).ok()
}

/// JS listener script forwarding `lotus-graph-event` notifications to Rust.
///
/// Replaces any earlier handler registered by a prior `/network` mount so
/// switching views back and forth never accumulates duplicate window listeners.
pub const fn listener_script() -> &'static str {
    r"
    (function () {
      if (window.__lotusGraphBridgeHandler) {
        window.removeEventListener('lotus-graph-event', window.__lotusGraphBridgeHandler);
      }
      var handler = function (event) {
        if (event && event.detail) {
          dioxus.send(event.detail);
        }
      };
      window.__lotusGraphBridgeHandler = handler;
      window.addEventListener('lotus-graph-event', handler);
      if (window.LotusGraphGL) {
        dioxus.send({ type: 'script_loaded' });
      }
    })();
    "
}

fn eval_gl_method(method: &str, args_js: &str) {
    let script = format!("if (window.LotusGraphGL) {{ window.LotusGraphGL.{method}({args_js}); }}");
    document::eval(&script);
}

/// Initialize the WebGL2 renderer on `canvas_id` and attach DOM labels under `labels_id`.
pub fn mount_graph(canvas_id: &str, labels_id: &str, native_download: bool) {
    let Ok(canvas_json) = serde_json::to_string(canvas_id) else {
        return;
    };
    let Ok(labels_json) = serde_json::to_string(labels_id) else {
        return;
    };
    let opts_json = if native_download {
        r#"{"nativeDownload":true}"#
    } else {
        r#"{"nativeDownload":false}"#
    };
    eval_gl_method(
        "mount",
        &format!("{canvas_json}, {labels_json}, {opts_json}"),
    );
}

/// Upload a complete compact flat [`GraphPayload`] to the WebGL2 GPU buffers.
pub fn push_graph_payload(payload: &GraphPayload) {
    let Ok(json) = serde_json::to_string(payload) else {
        return;
    };
    eval_gl_method("setGraph", &json);
}

/// Switch between `"dark"` ("Deep Observatory") and `"light"` ("Botanical Atlas").
pub fn push_theme(dark: bool) {
    let theme = if dark { r#""dark""# } else { r#""light""# };
    eval_gl_method("setTheme", theme);
}

/// Switch between `"3d"` and `"2d"` camera/projection modes.
pub fn push_mode(is_3d: bool) {
    let mode = if is_3d { r#""3d""# } else { r#""2d""# };
    eval_gl_method("setMode", mode);
}

/// Dim the live graph stage cleanly during an in-flight pivot query.
pub fn push_loading(loading: bool) {
    eval_gl_method("setLoading", if loading { "true" } else { "false" });
}

/// Highlight and frame `index` (or clear selection when `None`).
pub fn push_select_node(index: Option<usize>) {
    match index {
        Some(idx) => eval_gl_method("selectNode", &idx.to_string()),
        None => eval_gl_method("selectNode", "null"),
    }
}

/// Adjust camera distance by multiplicative `factor` (`> 1` zooms in, `< 1` zooms out).
pub fn zoom_graph(factor: f32) {
    eval_gl_method("zoomBy", &format!("{factor:.4}"));
}

/// Reset camera orientation and re-frame the entire graph.
pub fn reset_graph_view() {
    eval_gl_method("resetView", "");
}

/// Pause or resume the background force layout simulation.
pub fn push_paused(paused: bool) {
    eval_gl_method("setPaused", if paused { "true" } else { "false" });
}

/// Request a composite PNG snapshot of the canvas and floating DOM labels.
pub fn trigger_export_png() {
    eval_gl_method("exportPng", "");
}

/// Release the WebGL2 context, animation loop, observers, and DOM listeners.
pub fn destroy_graph() {
    eval_gl_method("destroy", "");
}

/// Focus `element_id` when focus is not already inside an editable form field.
pub fn focus_finder_if_not_typing(element_id: &str) {
    let Ok(id_json) = serde_json::to_string(element_id) else {
        return;
    };
    let script = format!(
        r"
        var active = document.activeElement;
        if (active && /^(INPUT|TEXTAREA|SELECT)$/i.test(active.tagName)) return;
        if (active && active.isContentEditable) return;
        var el = document.getElementById({id_json});
        if (el) {{ el.focus(); el.select && el.select(); }}
        "
    );
    document::eval(&script);
}
