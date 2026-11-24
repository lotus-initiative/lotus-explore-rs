// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The "Network" tab: lazy WebGL2 compound–taxon–reference graph workspace.

use crate::components::results_graph::ResultsGraphWorkspace;
use crate::document_head::GraphScripts;
use crate::hooks::use_locale::use_locale;
use crate::i18n::view_label_network;
use dioxus::prelude::*;

/// Full-bleed lazy page rendering the WebGL2 network workspace (`/network`).
#[component(lazy)]
pub fn NetworkPage() -> Element {
    let locale = use_locale();
    rsx! {
        GraphScripts {}
        section {
            class: "w-full max-w-none px-4 pt-3 sm:px-6 lg:px-8",
            aria_labelledby: "network-page-heading",
            h2 { class: "sr-only", id: "network-page-heading", "{view_label_network(locale)}" }
            div { class: "w-full rounded-xl border border-shell-border bg-shell-raised p-4 sm:p-5 overflow-x-clip",
                ResultsGraphWorkspace {}
            }
        }
    }
}
