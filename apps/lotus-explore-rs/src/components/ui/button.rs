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
    // Only the attributes that were given. An empty `aria-label=""` is not the
    // absence of a label: it overrides the visible text as the accessible name,
    // so a button with a label on it and an empty `aria-label` on it is announced
    // as nothing. Every one of these was `unwrap_or_default()` before, which
    // rendered all five on every button in the app.
    let optional: Vec<dioxus_core::Attribute> = [
        ("title", props.title.clone()),
        ("aria-label", props.aria_label.clone()),
        ("aria-controls", props.aria_controls.clone()),
        ("aria-expanded", props.aria_expanded.clone()),
        ("aria-pressed", props.aria_pressed.clone()),
    ]
    .into_iter()
    .filter_map(|(name, value)| {
        let value = value.filter(|value| !value.trim().is_empty())?;
        Some(Attribute::new(name, value, None, true))
    })
    .collect::<Vec<dioxus_core::Attribute>>();

    rsx! {
        button {
            r#type: props.r#type,
            disabled: props.disabled || props.loading,
            class: class,
            onclick: move |evt| {
                if !props.disabled && !props.loading
                    && let Some(handler) = props.onclick.as_ref() {
                        handler.call(evt);
                    }
            },
            // Last among the attributes: a spread ends the attribute list as far
            // as the macro is concerned, so anything after it is read as a child.
            ..optional,
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
    use super::{Button, POINTER, with_pointer};
    use dioxus::prelude::*;

    /// An attribute the caller did not ask for must not appear at all.
    ///
    /// This is the regression the spread exists to prevent. Every optional
    /// attribute used to render as `unwrap_or_default()`, so every button in the
    /// app carried `aria-label=""` --- and an empty `aria-label` is not the
    /// absence of a label: it overrides the visible text as the accessible name,
    /// so a labelled button was announced as nothing.
    #[component]
    fn Plain() -> Element {
        rsx! { Button { label: Some("Search".to_string()) } }
    }

    /// A button that does set them.
    #[component]
    fn Described() -> Element {
        rsx! {
            Button {
                label: Some("Filters".to_string()),
                title: Some("Filter the results".to_string()),
                aria_label: Some("Filter the results".to_string()),
                aria_pressed: Some("true".to_string()),
            }
        }
    }

    fn render_of(component: fn() -> Element) -> String {
        let mut dom = VirtualDom::new(component);
        dom.rebuild_in_place();
        dioxus_ssr::render(&dom)
    }

    #[test]
    fn an_unset_optional_attribute_is_absent_rather_than_empty() {
        let html = render_of(Plain);
        for attribute in [
            "aria-label",
            "aria-pressed",
            "aria-expanded",
            "aria-controls",
            "title",
        ] {
            assert!(
                !html.contains(attribute),
                "{attribute} was rendered for a button that did not set it:\n{html}"
            );
        }
        assert!(
            html.contains(">Search<"),
            "and the visible label, which is the accessible name, survives:\n{html}"
        );
    }

    #[test]
    fn a_set_optional_attribute_is_rendered() {
        let html = render_of(Described);
        assert!(
            html.contains(r#"aria-label="Filter the results""#),
            "{html}"
        );
        assert!(html.contains(r#"aria-pressed="true""#), "{html}");
        assert!(html.contains(r#"title="Filter the results""#), "{html}");
    }

    /// An attribute set to whitespace is the same as not set.
    ///
    /// `title: Some(" ".to_string())` renders a tooltip of nothing on hover and
    /// announces as nothing, so it is treated as absent rather than shipped.
    #[component]
    fn Blank() -> Element {
        rsx! {
            Button {
                label: Some("Search".to_string()),
                aria_label: Some("   ".to_string()),
            }
        }
    }

    #[test]
    fn a_blank_optional_attribute_is_treated_as_absent() {
        let html = render_of(Blank);
        assert!(!html.contains("aria-label"), "{html}");
    }

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
