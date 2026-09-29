// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared notice bar component using inline Tailwind utility classes.

use dioxus::prelude::*;

/// Visual tone for a notice bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeTone {
    Neutral,
    Warning,
}

/// Properties for the [`NoticeBar`] component.
#[derive(Clone, Props, Debug, PartialEq)]
pub struct NoticeBarProps {
    pub label: String,
    #[props(default = NoticeTone::Neutral)]
    pub tone: NoticeTone,
    #[props(default = "status")]
    pub role: &'static str,
    #[props(default = "polite")]
    pub aria_live: &'static str,
    #[props(default = false)]
    pub dark: bool,
    #[props(default)]
    pub trailing: Option<Element>,
    #[props(default)]
    pub children: Option<Element>,
}

#[component]
pub fn NoticeBar(props: NoticeBarProps) -> Element {
    let (outer_tone, label_tone) = match props.tone {
        NoticeTone::Neutral => (
            "border-border bg-panel-soft border-l-4 border-l-accent",
            "bg-accent/12 text-accent",
        ),
        NoticeTone::Warning => (
            "border-warning/35 bg-warning/10 border-l-4 border-l-warning",
            "bg-warning/12 text-warning",
        ),
    };

    rsx! {
        div {
            role: props.role,
            aria_live: props.aria_live,
            class: "notice-bar flex flex-wrap items-baseline gap-2 rounded-xl border p-2.5 shadow-xs text-ui {outer_tone}",
            span {
                class: "inline-flex items-center px-2 py-0.5 rounded-full font-bold uppercase tracking-[0.08em] text-micro shrink-0 whitespace-nowrap {label_tone}",
                "{props.label}"
            }
            if let Some(children) = props.children {
                div {
                    class: "flex flex-1 min-w-0 flex-wrap items-center gap-2 text-text",
                    {children}
                }
            }
            if let Some(trailing) = props.trailing {
                div {
                    class: "ml-auto flex flex-wrap items-center gap-2",
                    {trailing}
                }
            }
        }
    }
}
