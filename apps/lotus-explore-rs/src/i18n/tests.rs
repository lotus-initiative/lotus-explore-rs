// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `i18n`, in their own file.

use super::{Locale, TextKey, msg_tsv_missing_column, t};

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
