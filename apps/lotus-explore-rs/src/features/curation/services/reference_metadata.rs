// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

fn parse_quickstatements_text(text: &str) -> Option<Vec<String>> {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    (!lines.is_empty()).then_some(lines)
}

#[cfg(target_arch = "wasm32")]
pub(super) async fn fetch_reference_quickstatements(doi: &str) -> Option<Vec<String>> {
    use js_sys::{Function, Promise, Reflect};
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;

    let window = web_sys::window()?;
    let window_value = JsValue::from(window);
    let bridge = Reflect::get(&window_value, &JsValue::from_str("__lotusCitation")).ok()?;

    if bridge.is_null() || bridge.is_undefined() {
        return None;
    }

    let function = Reflect::get(&bridge, &JsValue::from_str("quickStatements"))
        .ok()?
        .dyn_into::<Function>()
        .ok()?;
    let result = function.call1(&bridge, &JsValue::from_str(doi)).ok()?;
    let result = if let Ok(promise) = result.clone().dyn_into::<Promise>() {
        JsFuture::from(promise).await.ok()?
    } else {
        result
    };

    parse_quickstatements_text(&result.as_string()?)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) async fn fetch_reference_quickstatements(_doi: &str) -> Option<Vec<String>> {
    let _ = parse_quickstatements_text("");
    None
}

#[cfg(test)]
#[path = "reference_metadata/tests.rs"]
mod tests;
