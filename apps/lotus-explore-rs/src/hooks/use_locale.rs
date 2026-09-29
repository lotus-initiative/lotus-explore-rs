// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Hooks to access and mutate locale throughout the component tree.

use crate::i18n::Locale;
use dioxus::prelude::*;

/// Read the current locale value from context.
/// Must be called inside a tree wrapped by [`super::LocaleProvider`].
/// Returns a plain `Locale` copy — subscribe only to locale changes.
#[must_use]
pub fn use_locale() -> Locale {
    let signal = use_context::<Signal<Locale>>();
    *signal.read()
}

/// Return the raw `Signal<Locale>` from context for write access.
/// Use this when a component needs to *change* the locale (e.g. `LangSwitch`).
/// For read-only access prefer [`use_locale`].
#[must_use]
pub fn use_locale_signal() -> Signal<Locale> {
    use_context::<Signal<Locale>>()
}
