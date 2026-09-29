// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Makes `Locale` available throughout the component tree.

use crate::i18n::Locale;
use dioxus::prelude::*;

/// Provider component that supplies `Locale` to descendants.
/// Wrap your app or a subtree with this to enable `use_locale()` hook access.
#[component]
pub fn LocaleProvider(locale: Signal<Locale>, children: Element) -> Element {
    use_context_provider(|| locale);
    rsx! {
        {children}
    }
}
