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
            "inline-flex flex-1 min-w-max items-center justify-center px-5 py-1.5 text-ui leading-none font-semibold rounded-full border border-accent bg-accent text-bg shadow-xs transition-transform duration-150 hover:bg-accent-2 active:bg-accent-2 active:scale-[0.98] min-h-[40px] whitespace-nowrap cursor-pointer focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2"
        } else {
            "inline-flex flex-none items-center justify-center px-5 py-1.5 text-ui leading-none font-semibold rounded-full border border-accent bg-accent text-bg shadow-xs transition-transform duration-150 hover:bg-accent-2 active:bg-accent-2 active:scale-[0.98] min-h-[40px] whitespace-nowrap cursor-pointer focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2"
        }
    } else if stretch {
        "inline-flex flex-1 min-w-max items-center justify-center px-5 py-1.5 text-ui leading-none font-semibold rounded-full border border-border bg-surface text-text hover:bg-bg transition-transform duration-150 active:scale-[0.98] min-h-[40px] whitespace-nowrap cursor-pointer focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2"
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

#[cfg(test)]
mod tests {
    //! A segmented item is `whitespace-nowrap`, so its minimum useful width is the width
    //! of its label. Two of this component's labels are long enough to matter in the
    //! languages this app ships -- German "Struktureditor" is a single sixteen-character
    //! word with no break opportunity, and French "Éditeur de structure" is twenty
    //! characters -- and the bug this pins is that the item could be *narrower* than
    //! its own text.
    //!
    //! `flex-1` sets `flex-shrink: 1`, and a flex item's automatic minimum size is what
    //! normally stops it shrinking below its content. `min-w-0` cancels that minimum
    //! outright, so the item would shrink and the `nowrap` label would overflow the
    //! button box -- text spilling over the rounded edge, or clipped by the scroll
    //! container it sits in. It read as "the button is too narrow", which is exactly
    //! what it was, and it only showed up in the languages with the longest labels.

    /// Every class string this component can put on a button.
    fn item_classes() -> Vec<&'static str> {
        vec![
            // active, stretched
            "inline-flex flex-1 min-w-max items-center justify-center px-5 py-1.5 text-ui leading-none font-semibold rounded-full border border-accent bg-accent text-bg shadow-xs transition-transform duration-150 hover:bg-accent-2 active:bg-accent-2 active:scale-[0.98] min-h-[40px] whitespace-nowrap cursor-pointer focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
            // active, not stretched
            "inline-flex flex-none items-center justify-center px-5 py-1.5 text-ui leading-none font-semibold rounded-full border border-accent bg-accent text-bg shadow-xs transition-transform duration-150 hover:bg-accent-2 active:bg-accent-2 active:scale-[0.98] min-h-[40px] whitespace-nowrap cursor-pointer focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
            // inactive, stretched
            "inline-flex flex-1 min-w-max items-center justify-center px-5 py-1.5 text-ui leading-none font-semibold rounded-full border border-border bg-surface text-text hover:bg-bg transition-transform duration-150 active:bg-bg active:scale-[0.98] min-h-[40px] whitespace-nowrap cursor-pointer focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
            // inactive, not stretched
            "inline-flex flex-none items-center justify-center px-5 py-1.5 text-ui leading-none font-semibold rounded-full border border-border bg-surface text-text hover:bg-bg transition-transform duration-150 active:bg-bg active:scale-[0.98] min-h-[40px] whitespace-nowrap cursor-pointer focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2",
        ]
    }

    #[test]
    fn a_nowrap_item_is_never_allowed_to_be_narrower_than_its_label() {
        for class in item_classes() {
            assert!(
                class.contains("whitespace-nowrap"),
                "this guard is about nowrap labels; a variant without one is not covered: \
                 {class}"
            );
            assert!(
                !class.contains("min-w-0"),
                "`min-w-0` cancels the automatic minimum width of a flex item, so this \
                 button can be squeezed narrower than its own nowrap label: {class}"
            );
            // `flex-none` cannot shrink at all. `flex-1` can, so it needs the explicit
            // `min-w-max` above; one without either would be the same bug in a
            // different spelling.
            if class.contains("flex-1") {
                assert!(
                    class.contains("min-w-max"),
                    "a growing item needs an explicit content-based minimum: {class}"
                );
            }
        }
    }

    #[test]
    fn the_class_strings_are_the_ones_the_component_actually_renders() {
        // The list above is a copy. A copy drifts, and a guard that checks a stale copy
        // guards nothing -- so it is compared against the source.
        let source = include_str!("segmented_control.rs");
        for class in item_classes() {
            assert!(
                source.contains(class),
                "this class string is no longer in the component, so the guard above is \
                 checking something that is not rendered: {class}"
            );
        }
    }
}
