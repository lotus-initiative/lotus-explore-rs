// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Ketcher molecule editor panel.

use crate::i18n::{TextKey, t};
use crate::ui::prelude::{NoticeBar, NoticeTone};
use dioxus::prelude::*;

/// Marks the "the editor is not in this build" error, so the panel can tell it
/// apart from a genuine failure and say the useful thing.
const EDITOR_NOT_BUNDLED: &str = "the structure editor is not in this build";

/// Where the editor has got to.
///
/// The first launch has to unpack 31 MB, so there is a real pending state. It
/// used to be skipped by rendering the frame immediately, which is what made a
/// missing or unpackable editor look like a silent empty box.
#[derive(Clone, PartialEq)]
enum EditorState {
    /// The editor is being unpacked, or the first render has not resolved it yet.
    Loading,
    /// In this build, and this is the URL the frame is given.
    Ready(String),
    /// Not in this build at all, so there is nothing to show and nothing to
    /// fetch. Kept apart from `Failed` because it is a build problem with a known
    /// remedy, and the user gets the remedy rather than a diagnostic.
    NotBundled,
    /// Present but unusable, and this is why.
    Failed(String),
}

impl EditorState {
    fn is_settled(&self) -> bool {
        !matches!(self, Self::Loading)
    }
}

#[component]
pub fn KetcherPanel() -> Element {
    let locale = crate::hooks::use_locale();
    let mut ketcher_ready = use_signal(|| false);
    // The frame cannot be pointed at the bundled path on a desktop build -- see
    // `desktop_assets` for what the navigation handler does to it -- so the URL is
    // resolved asynchronously and the panel shows a real state while it happens.
    // That is also why a missing editor is visible instead of an empty box.
    let editor = use_signal(|| EditorState::Loading);
    let bundled = crate::vendor_assets::ketcher_url();

    use_effect(move || {
        if editor.read().is_settled() {
            return;
        }
        let entry = bundled.clone();
        let mut editor = editor;
        spawn(async move {
            #[cfg(not(target_arch = "wasm32"))]
            let resolved = match entry {
                Some(entry) => crate::desktop_assets::ketcher_frame_url(&entry).await,
                None => Err(String::from(EDITOR_NOT_BUNDLED)),
            };
            // A browser serves the whole `public/` tree and has no navigation
            // handler, so the bundled path is already the right URL there.
            #[cfg(target_arch = "wasm32")]
            let resolved: Result<String, String> =
                entry.ok_or_else(|| String::from("the structure editor is not in this build"));

            match resolved {
                // Logged because a wrong URL looks exactly like every other
                // failure here: no frame, no message.
                Ok(url) => {
                    log::info!("event=ketcher_load state=url url={url}");
                    editor.set(EditorState::Ready(url));
                }
                Err(reason) => {
                    log::error!("event=ketcher_load state=error reason={reason}");
                    editor.set(if reason == EDITOR_NOT_BUNDLED {
                        EditorState::NotBundled
                    } else {
                        EditorState::Failed(reason)
                    });
                }
            }
        });
    });

    let ketcher_url = match editor.read().clone() {
        EditorState::NotBundled => {
            return rsx! {
                NoticeBar { tone: NoticeTone::Warning, label: t(locale, TextKey::KetcherNotBundled).to_string() }
            };
        }
        EditorState::Failed(reason) => {
            return rsx! {
                NoticeBar { tone: NoticeTone::Warning, label: reason }
            };
        }
        EditorState::Loading => {
            return rsx! {
                NoticeBar { tone: NoticeTone::Neutral, label: t(locale, TextKey::KetcherPreparing).to_string() }
            };
        }
        EditorState::Ready(url) => url,
    };

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
