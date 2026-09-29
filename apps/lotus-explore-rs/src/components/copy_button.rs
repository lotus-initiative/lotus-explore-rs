// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Reusable "copy to clipboard" button.

use crate::components::ui::Button;
use crate::i18n::{Locale, TextKey, t};
use dioxus::prelude::*;
use std::sync::Arc;

#[component]
pub fn CopyButton(
    text: Arc<str>,
    #[props(default = "")] label: &'static str,
    #[props(default = "")] title: &'static str,
    #[props(default = "")] class: &'static str,
    #[props(default = Locale::En)] locale: Locale,
) -> Element {
    let mut copied = use_signal(|| false);
    let label_attr = if label.is_empty() {
        t(locale, TextKey::Copy)
    } else {
        label
    };
    let title_attr = if title.is_empty() {
        t(locale, TextKey::CopyToClipboard)
    } else {
        title
    };

    rsx! {
        Button {
            r#type: "button",
            title: Some(title_attr.to_string()),
            aria_label: Some(title_attr.to_string()),
            class: "inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl border border-border bg-surface text-text font-semibold shadow-xs hover:bg-bg active:bg-bg min-h-[34px] gap-1.5 px-3 py-1.5 text-ui active:scale-[0.98] {class}",
            onclick: move |_| {
                copy_to_clipboard(text.as_ref());
                *copied.write() = true;
                spawn(async move {
                    gloo_timer_sleep_ms(1200).await;
                    *copied.write() = false;
                });
            },
            if *copied.read() {
                span { class: "font-semibold text-success", "aria-live": "polite", "✓ {t(locale, TextKey::Copied)}" }
            } else {
                span { "{label_attr}" }
            }
        }
    }
}

// `async` is required for signature parity at the call site: the WASM branch
// `.await`s a `JsFuture`, while the native branch sleeps synchronously. Clippy
// sees only the current target, so on native this "looks" like an unused async.
#[allow(clippy::unused_async)]
async fn gloo_timer_sleep_ms(ms: u32) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        use wasm_bindgen::closure::Closure;
        let promise = js_sys::Promise::new(&mut |resolve, _reject| {
            let cb = Closure::once_into_js(move || {
                let _ = resolve.call0(&wasm_bindgen::JsValue::NULL);
            });
            if let Some(win) = web_sys::window() {
                let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    i32::try_from(ms).unwrap_or(i32::MAX),
                );
            }
        });
        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::sleep(std::time::Duration::from_millis(u64::from(ms)));
    }
}

pub fn copy_to_clipboard(text: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;

        let Some(window) = web_sys::window() else {
            return;
        };
        let Some(document) = window.document() else {
            return;
        };

        let window_js = wasm_bindgen::JsValue::from(window);
        let nav = js_sys::Reflect::get(&window_js, &wasm_bindgen::JsValue::from_str("navigator"));
        if let Ok(nav) = nav
            && let Ok(clipboard) =
                js_sys::Reflect::get(&nav, &wasm_bindgen::JsValue::from_str("clipboard"))
            && let Ok(write_text) =
                js_sys::Reflect::get(&clipboard, &wasm_bindgen::JsValue::from_str("writeText"))
            && let Some(func) = write_text.dyn_ref::<js_sys::Function>()
        {
            let _ = func.call1(&clipboard, &wasm_bindgen::JsValue::from_str(text));
            return;
        }

        let area = document
            .create_element("textarea")
            .ok()
            .and_then(|el| el.dyn_into::<web_sys::HtmlTextAreaElement>().ok());
        if let (Some(ta), Some(body)) = (area, document.body()) {
            ta.set_value(text);
            let _ = ta.set_attribute("readonly", "");
            let _ = ta.set_attribute(
                "style",
                "position:fixed;top:0;left:0;opacity:0;pointer-events:none;",
            );
            let _ = body.append_child(&ta);
            ta.select();
            let doc_js = wasm_bindgen::JsValue::from(document);
            if let Ok(exec_cmd) =
                js_sys::Reflect::get(&doc_js, &wasm_bindgen::JsValue::from_str("execCommand"))
                && let Some(func) = exec_cmd.dyn_ref::<js_sys::Function>()
            {
                let _ = func.call1(&doc_js, &wasm_bindgen::JsValue::from_str("copy"));
            }
            let _ = body.remove_child(&ta);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = text;
    }
}
