// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! A group of buttons, one of which is selected.
//!
//! This is a `role="group"`, not a navigation landmark, and the two callers
//! each wrap it in a `nav`. It used to carry its own `aria-label` as well,
//! which gave the header two nested landmarks with the same accessible name --
//! a screen reader announced "Search Curation Structure editor" twice. The
//! label is still required here, on the group, because the group's name is what
//! distinguishes it from the other group in the header once both are inside
//! their own `nav`.

use dioxus::prelude::*;

/// Item rendered inside a segmented control.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentedControlItem {
    pub label: &'static str,
    pub value: &'static str,
}

/// Properties for the [`SegmentedControl`] component.
#[derive(Clone, Props, Debug, PartialEq)]
pub struct SegmentedControlProps {
    /// The group's accessible name.
    ///
    /// Pass an empty string when the caller already labels the surrounding
    /// landmark: two nested elements with the same name are announced twice,
    /// and a screen-reader user hears the header's section switcher listed
    /// under two identical headings.
    pub aria_label: &'static str,
    pub selected_value: &'static str,
    pub items: Vec<SegmentedControlItem>,
    pub on_select: EventHandler<String>,
    #[props(default = false)]
    pub dark: bool,
    #[props(default = false)]
    pub stretch: bool,
    #[props(default = true)]
    pub wrap: bool,
    #[props(default = "true")]
    pub active_aria_current: &'static str,
}

#[component]
pub fn SegmentedControl(props: SegmentedControlProps) -> Element {
    let stretch = props.stretch;
    let wrap = props.wrap;
    let on_select = props.on_select;

    rsx! {
        div {
            role: "group",
            // An empty name is not a name: `aria-label=""` is what removes one,
            // where a missing attribute would fall back to the contents.
            aria_label: if props.aria_label.is_empty() { None } else { Some(props.aria_label) },
            class: if wrap {
                "inline-flex flex-wrap items-center gap-1 shrink-0"
            } else {
                "inline-flex items-center gap-1 shrink-0"
            },
            for item in &props.items {
                SegmentedButton {
                    label: item.label,
                    value: item.value,
                    selected_value: props.selected_value,
                    stretch,
                    active_aria_current: props.active_aria_current,
                    on_select,
                }
            }
        }
    }
}

#[derive(Clone, Props, Debug, PartialEq)]
struct SegmentedButtonProps {
    pub label: &'static str,
    pub value: &'static str,
    pub selected_value: &'static str,
    pub on_select: EventHandler<String>,
    #[props(default = false)]
    pub stretch: bool,
    #[props(default = "true")]
    pub active_aria_current: &'static str,
}

#[component]
fn SegmentedButton(props: SegmentedButtonProps) -> Element {
    let active = props.value == props.selected_value;
    let stretch = props.stretch;
    let on_select = props.on_select;
    let value = props.value;
    let label = props.label;

    let class = if active {
        if stretch {
            "inline-flex flex-1 min-w-0 items-center justify-center px-5 py-1.5 text-ui leading-none font-semibold rounded-full border border-accent bg-accent text-bg shadow-xs transition-transform duration-150 hover:bg-accent-2 active:bg-accent-2 active:scale-[0.98] min-h-[40px] whitespace-nowrap cursor-pointer focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2"
        } else {
            "inline-flex flex-none items-center justify-center px-5 py-1.5 text-ui leading-none font-semibold rounded-full border border-accent bg-accent text-bg shadow-xs transition-transform duration-150 hover:bg-accent-2 active:bg-accent-2 active:scale-[0.98] min-h-[40px] whitespace-nowrap cursor-pointer focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2"
        }
    } else if stretch {
        "inline-flex flex-1 min-w-0 items-center justify-center px-5 py-1.5 text-ui leading-none font-semibold rounded-full border border-border bg-surface text-text hover:bg-bg transition-transform duration-150 active:scale-[0.98] min-h-[40px] whitespace-nowrap cursor-pointer focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2"
    } else {
        "inline-flex flex-none items-center justify-center px-5 py-1.5 text-ui leading-none font-semibold rounded-full border border-border bg-surface text-text hover:bg-bg transition-transform duration-150 active:scale-[0.98] min-h-[40px] whitespace-nowrap cursor-pointer focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2"
    };

    rsx! {
        button {
            r#type: "button",
            "data-segmented-value": "{value}",
            aria_pressed: if active { "true" } else { "false" },
            aria_current: if active { props.active_aria_current } else { "false" },
            class: class,
             onclick: move |_| on_select.call(value.to_string()),
            "{label}"
        }
    }
}
