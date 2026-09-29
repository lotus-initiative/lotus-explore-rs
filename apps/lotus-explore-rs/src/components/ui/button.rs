// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared Button component using inline Tailwind classes.

use dioxus::prelude::*;

/// Props for the Button component.
#[derive(Props, Clone, PartialEq)]
pub struct ButtonProps {
    #[props(default)]
    pub label: Option<String>,
    #[props(default = "button")]
    pub r#type: &'static str,
    #[props(default)]
    pub disabled: bool,
    #[props(default)]
    pub loading: bool,
    #[props(default)]
    pub title: Option<String>,
    #[props(default)]
    pub aria_label: Option<String>,
    #[props(default)]
    pub aria_controls: Option<String>,
    #[props(default)]
    pub aria_expanded: Option<String>,
    #[props(default)]
    pub aria_pressed: Option<String>,
    #[props(into, default)]
    pub class: Option<String>,
    #[props(default)]
    pub onclick: Option<EventHandler<MouseEvent>>,
    #[props(default)]
    pub children: Element,
}

/// The pointer class.
/// Part of the component contract, not a styling choice: `class` replaces
/// `default_class` wholesale, so callers passing their own class would
/// otherwise lose the pointer.
const POINTER: &str = "cursor-pointer";

/// Append [`POINTER`] unless the caller's class already carries it.
fn with_pointer(class: &str) -> String {
    if class.split_whitespace().any(|token| token == POINTER) {
        class.to_owned()
    } else {
        format!("{class} {POINTER}")
    }
}

#[component]
pub fn Button(props: ButtonProps) -> Element {
    let default_class = "inline-flex items-center justify-center font-sans select-none transition-transform duration-150 ease-[cubic-bezier(.4,0,.2,1)] focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-accent/28 focus-visible:ring-offset-2 rounded-xl bg-accent text-bg font-semibold shadow-xs hover:bg-accent-2 active:bg-accent-2 min-h-[40px] gap-2 px-3.5 py-2 text-ui active:scale-[0.98] cursor-pointer";
    let class = with_pointer(props.class.as_deref().unwrap_or(default_class));

    rsx! {
        button {
            r#type: props.r#type,
            disabled: props.disabled || props.loading,
            title: props.title.as_deref().unwrap_or_default(),
            aria_label: props.aria_label.as_deref().unwrap_or_default(),
            aria_controls: props.aria_controls.as_deref().unwrap_or_default(),
            aria_expanded: props.aria_expanded.as_deref().unwrap_or_default(),
            aria_pressed: props.aria_pressed.as_deref().unwrap_or_default(),
            class: class,
            onclick: move |evt| {
                if !props.disabled && !props.loading
                    && let Some(handler) = props.onclick.as_ref() {
                        handler.call(evt);
                    }
            },
            if props.loading {
                span {
                    class: "inline-block size-3.5 rounded-full border-2 border-current border-t-transparent animate-spin",
                    "aria-hidden": "true",
                }
            }
            if let Some(ref text) = props.label {
                span { "{text}" }
            }
            {props.children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{POINTER, with_pointer};

    #[test]
    fn pointer_is_added_when_the_caller_omits_it() {
        assert_eq!(
            with_pointer("inline-flex rounded-xl bg-accent"),
            format!("inline-flex rounded-xl bg-accent {POINTER}")
        );
    }

    #[test]
    fn pointer_is_not_duplicated() {
        let once = with_pointer("rounded-xl");
        assert_eq!(with_pointer(&once), once);
    }

    #[test]
    fn an_existing_pointer_survives_a_substring_lookalike() {
        // `cursor-pointer-events` is a different utility; only the exact token counts.
        let class = "rounded-xl cursor-pointer-events";
        assert_eq!(with_pointer(class), format!("{class} {POINTER}"));
    }
}
