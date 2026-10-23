// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `i18n`, in their own file.

use super::{Locale, TextKey, err_truncated_by_endpoint, msg_tsv_missing_column, t};
use lotus_explore_core::explore::truncation::Reason;

#[test]
fn landing_copy_uses_chemical_entities_in_every_locale() {
    let expected = [
        (Locale::En, "chemical entities"),
        (Locale::Fr, "entités chimiques"),
        (Locale::De, "chemische Entitäten"),
        (Locale::It, "entità chimiche"),
    ];
    for (locale, phrase) in expected {
        assert!(t(locale, TextKey::PageSubtitle).contains(phrase));
        assert!(t(locale, TextKey::WelcomeLeadA).contains(phrase));
    }
}

#[test]
fn tsv_missing_column_messages_are_localized() {
    for locale in [Locale::En, Locale::Fr, Locale::De, Locale::It] {
        let message = msg_tsv_missing_column(locale, "name");
        // `contains` already implies non-empty, so this asserts the column
        // name survived substitution in every locale, which is the thing that
        // can actually break.
        assert!(
            message.contains("name"),
            "{locale:?} dropped the column name: {message:?}"
        );
    }
}

#[test]
fn every_truncation_reason_is_translated_in_every_locale() {
    // The two strings `lotus-query` puts around its own wording. A rendered
    // message that still contains them is English inside a translated frame.
    const NOTICE_CLAUSE: &str = "the endpoint stopped sending before the result was complete";
    const REASON_OPEN: &str = "(the endpoint said: ";
    const LOCALES: [Locale; 4] = [Locale::En, Locale::Fr, Locale::De, Locale::It];

    for locale in LOCALES {
        for reason in [Reason::TimedOut, Reason::Cancelled, Reason::Other] {
            let message = err_truncated_by_endpoint(locale, reason);
            assert!(
                !message.contains(REASON_OPEN) && !message.contains(NOTICE_CLAUSE),
                "{locale:?}/{reason:?} still carries the raw detail: {message}"
            );
            assert!(
                !message.contains('{') && !message.contains('}'),
                "{locale:?}/{reason:?} has an unsubstituted placeholder: {message}"
            );
            assert!(message.chars().count() > 40, "{locale:?}: {message}");
        }
    }

    // And the four locales are actually different from each other, so a missing
    // translation cannot pass as one.
    let rendered: Vec<String> = LOCALES
        .iter()
        .map(|locale| err_truncated_by_endpoint(*locale, Reason::TimedOut))
        .collect();
    for (i, a) in rendered.iter().enumerate() {
        for b in rendered.iter().skip(i + 1) {
            assert_ne!(a, b, "two locales render the same truncation message");
        }
    }
}
