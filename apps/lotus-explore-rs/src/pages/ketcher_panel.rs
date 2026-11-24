// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Ketcher molecule editor panel.
//!
//! The editor is a separate static web application served under `public/`, so
//! the bundled path resolves directly as the iframe `src` URL.

use crate::i18n::{TextKey, t};
use crate::ui::notice::{NoticeBar, NoticeTone};
use dioxus::prelude::*;

/// Log reason recorded when the Ketcher assets are absent from this build.
const EDITOR_NOT_BUNDLED: &str = "the structure editor is not in this build";

/// Where the editor has got to.
#[derive(Clone, PartialEq)]
enum EditorState {
    /// Initial render before resolving the bundled asset path.
    Loading,
    /// Bundled entry URL ready for the editor iframe.
    Ready(String),
    /// Not in this build at all, so there is nothing to show and nothing to fetch.
    NotBundled,
}

impl EditorState {
    fn is_settled(&self) -> bool {
        !matches!(self, Self::Loading)
    }
}

#[component]
pub fn KetcherPanel() -> Element {
    let locale = crate::hooks::use_locale::use_locale();
    let ketcher_ready = use_signal(|| false);
    let editor = use_signal(|| EditorState::Loading);

    {
        let bundled = crate::vendor_assets::ketcher_url();
        use_effect(move || {
            if editor.read().is_settled() {
                return;
            }
            let mut editor = editor;
            if let Some(url) = bundled.clone() {
                log::info!("event=ketcher_load state=ready bytes={}", url.len());
                editor.set(EditorState::Ready(url));
            } else {
                log::error!("event=ketcher_load state=error reason={EDITOR_NOT_BUNDLED}");
                editor.set(EditorState::NotBundled);
            }
        });
    }

    panel(editor, locale, ketcher_ready)
}

fn panel(
    editor: Signal<EditorState>,
    locale: crate::i18n::Locale,
    ketcher_ready: Signal<bool>,
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
                class: "lotus-ketcher-frame w-full flex-1 border-0 bg-surface",
                allow: "fullscreen",
                referrerpolicy: "no-referrer",
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
                p { class: "max-w-[70ch] text-body leading-relaxed text-muted px-1",
                    "{t(locale, TextKey::KetcherHintA)}"
                    strong { class: "text-text", "{t(locale, TextKey::KetcherSummary)}" }
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
                        r#type: "button",
                        class: "flex lotus-ketcher-frame w-full flex-1 cursor-pointer items-center justify-center rounded-xl border-0 bg-bg text-center hover:bg-panel-soft focus-visible:outline-2 focus-visible:outline-accent focus-visible:outline-offset-2",
                        onclick: move |_| ketcher_ready.set(true),
                        em {
                            class: "text-body text-muted",
                            "{t(locale, TextKey::KetcherClickToLoad)}"
                        }
                    }
                }
            }
        }
    }
}
