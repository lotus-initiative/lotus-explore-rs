// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Toolbar assembly for the results table.

use dioxus::prelude::*;

#[component]
pub(super) fn ResultsToolbar() -> Element {
    rsx! {
        div { class: "w-full max-w-none px-0",
            div { class: "flex w-full min-w-0 flex-col gap-4",
                super::table_toolbar_sections::QueryPanel {}
                super::table_toolbar_sections::StatBar {}
                super::table_toolbar_sections::DownloadActionsGroup {}
            }
        }
        super::table_toolbar_sections::CappedRowsNotice {}
    }
}
