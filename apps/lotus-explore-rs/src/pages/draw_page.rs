// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The "Structure editor" tab: a full-pane Ketcher molecule editor.

use crate::pages::ketcher_panel::KetcherPanel;
use dioxus::prelude::*;

#[component(lazy)]
pub fn DrawPage() -> Element {
    rsx! {
        section {
            class: "page-section w-full max-w-none px-4 sm:px-6 lg:px-8",
            div { class: "w-full rounded-xl border border-shell-border bg-shell-raised overflow-hidden",
                // Full-width shell here keeps the editor usable on large screens without a faux content column.
                KetcherPanel {}
            }
        }
    }
}
