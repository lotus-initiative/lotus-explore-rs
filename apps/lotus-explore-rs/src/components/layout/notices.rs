// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Status, warning, and error notice components.

use crate::components::copy_button::CopyButton;
use crate::components::ui::Button;
use crate::features::explore::interactions::use_explore_interactions;
use crate::features::explore::recovery;
use crate::features::explore::selectors::{use_lifecycle_selector, use_result_selector};
use crate::features::explore::types::ErrorKind;
use crate::features::explore::url_state::absolute_share_url;
use crate::i18n::{TextKey, t};
use crate::services::error_presenter::{
    error_hint_text, format_domain_error, format_taxon_warning,
};
use crate::state::{use_app_state_context, use_results_context};
use crate::ui::prelude::{NoticeBar, NoticeTone};
use dioxus::prelude::*;
use std::sync::Arc;

#[component]
pub fn ShareNotice(shareable_url: Memo<Option<Arc<str>>>) -> Element {
    let locale = crate::hooks::use_locale();
    let dark_mode = use_app_state_context().state.read().dark_mode;
    let share_input_id = "share-url-field";
    let share = shareable_url.read();
    let Some(share) = share.as_deref() else {
        return rsx! {};
    };
    rsx! {
        NoticeBar {
            label: t(locale, TextKey::Share).to_string(),
            tone: NoticeTone::Warning,
            role: "status",
            aria_live: "polite",
            dark: dark_mode,
            // Real <label for>: the old `aria-label` named the copy button
            // beside the field, and overrode the visible text besides.
            label {
                r#for: share_input_id,
                class: "shrink-0 text-ui font-semibold text-muted",
                "{t(locale, TextKey::ShareableLink)}"
            }
            input {
                id: share_input_id,
                name: "share_url",
                autocomplete: "off",
                r#type: "text",
                readonly: true,
                value: "{share}",
                class: "min-w-0 flex-1 truncate font-mono w-full rounded-xl border border-border bg-surface px-2 py-1.5 text-body text-text shadow-xs focus-visible:outline-none focus-visible:border-accent focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
            }
            CopyButton {
                text: Arc::<str>::from(absolute_share_url(share)),
                title: t(locale, TextKey::CopyShareableLink),
                locale,
            }
        }
    }
}

#[component]
pub fn TaxonNotice() -> Element {
    let locale = crate::hooks::use_locale();
    let dark_mode = use_app_state_context().state.read().dark_mode;
    let explore = use_results_context().explore;
    let notice = use_result_selector(explore, |result| result.taxon_notice.clone());
    let notice = notice.read();
    let Some(warning) = notice.as_ref() else {
        return rsx! {};
    };
    let text = format_taxon_warning(locale, warning);
    rsx! {
        NoticeBar {
            label: t(locale, TextKey::Notice).to_string(),
            tone: NoticeTone::Warning,
            role: "status",
            aria_live: "polite",
            dark: dark_mode,
            span { class: "flex-1 min-w-0 text-ui text-muted break-words leading-snug", "{text}" }
        }
    }
}

#[component]
pub fn ErrorNotice() -> Element {
    let locale = crate::hooks::use_locale();
    let dark_mode = use_app_state_context().state.read().dark_mode;
    let explore = use_results_context().explore;
    let interactions = use_explore_interactions();
    let retry_interactions = interactions.clone();
    let domain_error = use_lifecycle_selector(explore, |lifecycle| lifecycle.error.clone());
    let is_loading = use_lifecycle_selector(explore, |lifecycle| lifecycle.loading);
    let domain_error = domain_error.read();
    let Some(domain_err) = domain_error.as_ref() else {
        return rsx! {};
    };
    let kind: ErrorKind = domain_err.kind();
    let msg = format_domain_error(locale, domain_err);
    rsx! {
        NoticeBar {
            label: t(locale, TextKey::Error).to_string(),
            tone: NoticeTone::Warning,
            role: "alert",
            aria_live: "assertive",
            dark: dark_mode,
            span { class: "flex-1 min-w-0 text-ui text-muted break-words leading-snug", "{msg}" }
            span { class: "min-w-0 text-ui text-subtle break-words leading-snug", "{error_hint_text(locale, kind)}" }
            if recovery::should_show_retry_button(domain_err) && !*is_loading.read() {
                Button {
                    r#type: "button",
                    label: t(locale, TextKey::Retry).to_string(),
                    class: "inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl border border-border bg-surface text-text font-semibold shadow-xs hover:bg-bg active:bg-bg min-h-[34px] gap-1.5 px-3 py-1.5 text-ui active:scale-[0.98]",
                    onclick: move |_| retry_interactions.retry(),
                }
            }
            button {
                r#type: "button",
                aria_label: "{t(locale, TextKey::DismissError)}",
                class: "notice-dismiss flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-xl text-base font-bold text-subtle hover:bg-danger/15 hover:text-danger focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-danger/40 focus-visible:ring-offset-1",
                onclick: move |_| interactions.dismiss_error(),
                "×"
            }
        }
    }
}
