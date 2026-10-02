// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Ketcher molecule editor panel.
//!
//! The editor is a separate web application, so it loads in a frame. Where that
//! frame points differs by target, and the difference is not ours to choose:
//!
//! - **In a browser** the whole `public/` tree is served, so the bundled path is
//!   already a URL and the frame is a plain `src`. Nothing to work around.
//! - **In a desktop window** a frame cannot be navigated to anything that dioxus
//!   lets through, and the one URL it does let through has the wrong shape. The
//!   mechanism, and the four that fail, are written out in
//!   [`crate::desktop_assets`]. In short: an empty `about:blank` frame, the
//!   editor's document written into it from here, and a script that gives the
//!   document a real path before the editor's own bundle reads it.
//!
//! The states are real rather than optimistic because a frame that fails silently
//! looks exactly like a frame that is still loading, and the first launch has to
//! unpack 31 MB.

use crate::i18n::{TextKey, t};
use crate::ui::prelude::{NoticeBar, NoticeTone};
use dioxus::prelude::*;

/// Marks the "the editor is not in this build" error, so the panel can tell it
/// apart from a genuine failure and say the useful thing.
const EDITOR_NOT_BUNDLED: &str = "the structure editor is not in this build";

/// Where the editor has got to.
#[derive(Clone, PartialEq)]
enum EditorState {
    /// The editor is being unpacked, or the first render has not resolved it yet.
    Loading,
    /// In this build. On a browser this is a URL; on desktop it is the document to
    /// write into the frame, and the frame is `about:blank`.
    Ready(String),
    /// Not in this build at all, so there is nothing to show and nothing to fetch.
    /// Kept apart from `Failed` because it is a build problem with a known remedy,
    /// and the user gets the remedy rather than a diagnostic.
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
// The four desktop signals are declared on both targets so the hook count does not
// vary; three of them are only read by the desktop effects.
#[cfg_attr(target_arch = "wasm32", allow(unused_variables))]
pub fn KetcherPanel() -> Element {
    let locale = crate::hooks::use_locale();
    let ketcher_ready = use_signal(|| false);

    // Every signal, declared unconditionally and at the top. Two reasons, both
    // learned the hard way: a hook declared after the effect that writes to it is a
    // hook that effect cannot see -- which is how the document once went missing
    // between being fetched and written into the frame -- and a hook behind a
    // `#[cfg]` means a different number of hooks per target, which is the sort of
    // thing that works until it does not.
    //
    // The last four are only read by the desktop effects below. A browser frame is
    // a plain `src`: there is no document to hand over, no frame to wait for and
    // nothing to listen to. So they are declared and unused there, which is cheaper
    // than a per-target hook count.
    let editor = use_signal(|| EditorState::Loading);
    let document_to_write = use_signal(|| Option::<String>::None);
    let frame_loaded = use_signal(|| false);
    let placed = use_signal(|| false);
    let listening = use_signal(|| false);

    // Resolve what to point the frame at. In a browser that is a URL and it is
    // immediate, but it goes through the same states as the desktop path so there
    // is one code path to reason about.
    {
        let bundled = crate::vendor_assets::ketcher_url();
        use_effect(move || {
            if editor.read().is_settled() {
                return;
            }
            let bundled = bundled.clone();
            let mut editor = editor;
            #[cfg(not(target_arch = "wasm32"))]
            let mut document_to_write = document_to_write;

            spawn(async move {
                #[cfg(target_arch = "wasm32")]
                let resolved: Result<String, String> =
                    bundled.ok_or_else(|| String::from(EDITOR_NOT_BUNDLED));

                #[cfg(not(target_arch = "wasm32"))]
                let resolved: Result<String, String> = match bundled {
                    Some(entry) => crate::desktop_assets::ketcher_document(&entry).await,
                    None => Err(String::from(EDITOR_NOT_BUNDLED)),
                };

                match resolved {
                    // Logged because a wrong frame looks exactly like every other
                    // failure here: no editor, no message.
                    Ok(document) => {
                        log::info!("event=ketcher_load state=ready bytes={}", document.len());
                        #[cfg(not(target_arch = "wasm32"))]
                        document_to_write.set(Some(document.clone()));
                        editor.set(EditorState::Ready(document));
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
    }

    // Put the document in the frame once both exist. Ordering is the whole of it:
    // the fetch is the slow part and the frame is not there yet when it finishes,
    // so one has to wait for the other and neither can hold the result alone.
    #[cfg(not(target_arch = "wasm32"))]
    {
        use_effect(move || {
            if !*frame_loaded.read() || *placed.read() {
                return;
            }
            let Some(html) = document_to_write.read().clone() else {
                return;
            };
            // Exactly once, and it has to be explicit. Writing the document into
            // the frame loads a new document in it, which fires the frame's `load`
            // again, which re-enters this effect; a second write then fails,
            // because the first `eval` has already finished.
            let mut placed = placed;
            placed.set(true);
            let mut editor = editor;
            spawn(async move {
                match crate::desktop_assets::write_into_frame(&html).await {
                    Ok(()) => log::info!("event=ketcher_load state=placed bytes={}", html.len()),
                    Err(reason) => {
                        log::error!("event=ketcher_load state=place_failed reason={reason}");
                        editor.set(EditorState::Failed(reason));
                    }
                }
            });
        });

        // Listen for what the editor's own document reports about itself. A frame's
        // `onload` fires when the document *parses*, so an editor whose every chunk
        // 404s looks exactly like one that worked -- which is how the `blob:` and
        // loopback attempts both went unnoticed.
        use_effect(move || {
            if *listening.read() || !matches!(*editor.read(), EditorState::Ready(_)) {
                return;
            }
            let mut listening = listening;
            let mut editor = editor;
            listening.set(true);
            spawn(async move {
                let script = r#"window.addEventListener("message", (event) => {
                    if (event.data && event.data.lotus === "ketcher_probe") {
                        dioxus.send(event.data);
                    }
                });"#;
                let mut eval = document::eval(script);
                while let Ok(message) = eval.recv::<serde_json::Value>().await {
                    let state = message
                        .get("state")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("unknown");
                    let text = |key: &str| {
                        message
                            .get(key)
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .to_owned()
                    };
                    match state {
                        "prepared" => {
                            let took = message
                                .get("took")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false);
                            log::info!(
                                "event=ketcher_probe state=prepared path={} took={} {}",
                                text("path"),
                                took,
                                text("why")
                            );
                        }
                        "probed" => {
                            let mounted = message
                                .get("mounted")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false);
                            let children = message
                                .get("children")
                                .and_then(serde_json::Value::as_i64)
                                .unwrap_or(-1);
                            let scripts = message
                                .get("scripts")
                                .and_then(serde_json::Value::as_u64)
                                .unwrap_or_default();
                            if mounted {
                                log::info!(
                                    "event=ketcher_probe state=mounted children={children} scripts={scripts}"
                                );
                            } else {
                                log::error!(
                                    "event=ketcher_probe state=not_mounted children={children} scripts={scripts}"
                                );
                                editor.set(EditorState::Failed(String::from(
                                    "the structure editor loaded but did not start",
                                )));
                            }
                        }
                        _ => {
                            log::error!(
                                "event=ketcher_probe state={state} message={} source={} line={}",
                                text("message"),
                                text("source"),
                                message
                                    .get("line")
                                    .and_then(serde_json::Value::as_i64)
                                    .unwrap_or_default()
                            );
                        }
                    }
                }
            });
        });
    }

    // The effects above own the document, the frame and the listener; the markup
    // only needs to know the frame is there.
    #[cfg(not(target_arch = "wasm32"))]
    {
        panel(editor, locale, ketcher_ready, frame_loaded)
    }
    #[cfg(target_arch = "wasm32")]
    {
        panel(editor, locale, ketcher_ready, use_signal(|| false))
    }
}

/// The panel, for whichever target this is.
///
/// A browser serves the whole `public/` tree and has no navigation handler, so the
/// bundled path is already a URL and the frame is a plain `src`. A desktop window
/// has to go through four mechanisms that do not work before it reaches the one
/// that does; that is written out in [`crate::desktop_assets`] rather than here.
///
/// The two are separate functions rather than one tree with a `#[cfg]` on a node
/// because `rsx!` will not take one.
#[cfg(target_arch = "wasm32")]
fn panel(
    editor: Signal<EditorState>,
    locale: crate::i18n::Locale,
    ketcher_ready: Signal<bool>,
    _frame_loaded: Signal<bool>,
) -> Element {
    if let Some(notice) = pending_notice(&editor, locale) {
        return notice;
    }
    let url = match editor.read().clone() {
        EditorState::Ready(url) => url,
        _ => String::new(),
    };
    shell(
        locale,
        ketcher_ready,
        rsx! {
            iframe {
                src: "{url}",
                title: "{t(locale, TextKey::KetcherIframeTitle)}",
                class: "min-h-[420px] w-full flex-1 border-0 bg-surface",
                allow: "fullscreen",
                referrerpolicy: "no-referrer",
            }
        },
    )
}

/// The panel, in a desktop window.
#[cfg(not(target_arch = "wasm32"))]
fn panel(
    editor: Signal<EditorState>,
    locale: crate::i18n::Locale,
    ketcher_ready: Signal<bool>,
    mut frame_loaded: Signal<bool>,
) -> Element {
    if let Some(notice) = pending_notice(&editor, locale) {
        return notice;
    }
    shell(
        locale,
        ketcher_ready,
        rsx! {
            iframe {
                id: "{crate::desktop_assets::FRAME_ID}",
                // An empty frame, written into from the window that owns it. Every URL
                // that would work otherwise is intercepted: a `dioxus://` path by the
                // one-navigation policy, an `http://` one by the handler that opens it
                // in the user's browser.
                src: "about:blank",
                title: "{t(locale, TextKey::KetcherIframeTitle)}",
                class: "min-h-[420px] w-full flex-1 border-0 bg-surface",
                allow: "fullscreen",
                onload: move |_| {
                    // Only the first. Writing the document into the frame loads a
                    // document in it, and that fires `load` again.
                    if !*frame_loaded.read() {
                        log::info!("event=ketcher_load state=frame_loaded");
                        frame_loaded.set(true);
                    }
                },
            }
        },
    )
}

/// The notice to show instead of the frame, if the editor is not ready.
fn pending_notice(editor: &Signal<EditorState>, locale: crate::i18n::Locale) -> Option<Element> {
    match editor.read().clone() {
        EditorState::NotBundled => Some(rsx! {
            NoticeBar { tone: NoticeTone::Warning, label: t(locale, TextKey::KetcherNotBundled).to_string() }
        }),
        EditorState::Failed(reason) => Some(rsx! {
            NoticeBar { tone: NoticeTone::Warning, label: reason }
        }),
        EditorState::Loading => Some(rsx! {
            NoticeBar { tone: NoticeTone::Neutral, label: t(locale, TextKey::KetcherPreparing).to_string() }
        }),
        EditorState::Ready(_) => None,
    }
}

/// The panel's chrome, with the frame or the load button in the space below it.
///
/// Rendered already visible: Ketcher measures its canvas and menus on mount, and a
/// `display:none` host leaves those boxes unlaid out, which strands the editor
/// without an instance.
fn shell(locale: crate::i18n::Locale, mut ketcher_ready: Signal<bool>, frame: Element) -> Element {
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
                    {frame}
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
