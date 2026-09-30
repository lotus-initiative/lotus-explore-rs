// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Ketcher molecule editor panel.

use crate::i18n::{TextKey, t};
use dioxus::prelude::*;

#[component]
pub fn KetcherPanel() -> Element {
    let locale = crate::hooks::use_locale();
    let mut ketcher_ready = use_signal(|| false);
    let ketcher_url = crate::vendor_assets::ketcher_url();
    // Logged because this URL is wrong in exactly one way per bundler: a
    // relative path, a missing `assets/` prefix, or a hashed name the editor's
    // own relative references do not match. All three look like "the editor does
    // not load" and nothing else.
    log::info!("event=ketcher_load state=url url={ketcher_url}");

    rsx! {
        div {
            class: "flex w-full flex-col gap-3",
            div {
                class: "flex flex-col gap-3 p-4",
                p { class: "text-micro text-subtle px-1",
                    "{t(locale, TextKey::KetcherHintA)}"
                    strong { class: "text-muted", "{t(locale, TextKey::KetcherSummary)}" }
                    "{t(locale, TextKey::KetcherHintB)}"
                    em { "{t(locale, TextKey::EditCopyDaylightSmiles)}" }
                    "{t(locale, TextKey::KetcherHintC)}"
                    em { "{t(locale, TextKey::CopyExtendedSmilesMol)}" }
                    "{t(locale, TextKey::KetcherHintD)}"
                }
                if *ketcher_ready.read() {
                    // Rendered already visible: Ketcher measures its canvas and
                    // menus on mount, and a display:none host leaves those boxes
                    // unlaid out, which strands the editor without an instance.
                    iframe {
                        src: "{ketcher_url}",
                        title: "{t(locale, TextKey::KetcherIframeTitle)}",
                        class: "min-h-[420px] w-full flex-1 border-0 bg-surface",
                        allow: "fullscreen",
                        referrerpolicy: "no-referrer",
                        // The editor is a separate application in a frame, so
                        // nothing in this window's render tree reports on it. A
                        // 404, a MIME the WebView refuses, or a runtime error
                        // inside Ketcher all look the same from out here: an
                        // empty box.
                        onload: move |_| log::info!("event=ketcher_load state=iframe_loaded"),
                        onerror: move |_| log::warn!("event=ketcher_load state=iframe_failed"),
                    }
                } else {
                    button {
                        class: "flex min-h-[420px] w-full flex-1 cursor-pointer items-center justify-center border-0 bg-bg text-center hover:bg-panel-soft focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 focus-visible:rounded",
                        onclick: move |_| ketcher_ready.set(true),
                        em {
                            class: "text-micro text-subtle",
                            "{t(locale, TextKey::KetcherClickToLoad)}"
                        }
                    }
                }
            }
        }
    }
}
