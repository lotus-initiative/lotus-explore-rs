// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `i18n`, in their own file.

use super::{
    Locale, TextKey, aria_graph_kind_toggle, err_truncated_by_endpoint, graph_two_type_note,
    msg_tsv_missing_column, t,
};
use lotus_explore_core::explore::truncation::Reason;

#[test]
fn the_subtitle_says_what_a_search_returns_in_every_locale() {
    let expected = [
        (
            Locale::En,
            "Find which compounds are reported from an organism, in which articles, as recorded in Wikidata by the LOTUS initiative.",
        ),
        (
            Locale::Fr,
            "Trouvez quels composés sont rapportés dans un organisme, et dans quels articles, d'après les données réunies dans Wikidata par l'initiative LOTUS.",
        ),
        (
            Locale::De,
            "Finden Sie heraus, welche Verbindungen aus einem Organismus beschrieben wurden und in welchen Artikeln, so wie die LOTUS-Initiative es in Wikidata erfasst hat.",
        ),
        (
            Locale::It,
            "Trova quali composti sono riportati in un organismo, e in quali articoli, secondo i dati raccolti in Wikidata dall'iniziativa LOTUS.",
        ),
    ];
    for (locale, sentence) in expected {
        assert_eq!(t(locale, TextKey::PageSubtitle), sentence);
    }
}

#[test]
fn the_structure_modes_say_what_they_compare() {
    // A pasted structure is resolved by the structure service at a cutoff of
    // one, which is not an identity test: stereoisomers come back too.
    assert_eq!(
        t(Locale::En, TextKey::Exact),
        "Exact (a pasted structure can also match its stereoisomers)"
    );
    assert_eq!(
        super::threshold_label(Locale::En, 0.85),
        "Tanimoto similarity (IDSM structure service), at least 0.85"
    );
    assert_eq!(
        super::threshold_label(Locale::Fr, 0.85),
        "Similarité de Tanimoto (service de structures IDSM), au moins 0.85"
    );
    assert_eq!(
        super::threshold_label(Locale::De, 0.85),
        "Tanimoto-Ähnlichkeit (Strukturdienst IDSM), mindestens 0.85"
    );
    assert_eq!(
        super::threshold_label(Locale::It, 0.85),
        "Somiglianza di Tanimoto (servizio di strutture IDSM), almeno 0.85"
    );
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

#[test]
fn graph_two_type_note_and_legend_toggle_aria_are_localized_and_label_in_name() {
    const LOCALES: [Locale; 4] = [Locale::En, Locale::Fr, Locale::De, Locale::It];

    let mut single_notes = Vec::new();
    let mut multi_notes = Vec::new();
    for locale in LOCALES {
        let single = graph_two_type_note(locale, "Reference", 1, "1", "refs");
        let multi = graph_two_type_note(locale, "Reference", 4, "4", "refs");
        assert!(single.contains("Reference"), "{locale:?}: {single}");
        assert!(
            multi.contains("4 refs") && multi.contains("Reference"),
            "{locale:?}: {multi}"
        );
        assert!(
            !single.contains('—') && !multi.contains('—'),
            "{locale:?} must not use em dashes"
        );
        single_notes.push(single);
        multi_notes.push(multi);

        for (active, can_toggle) in [(true, true), (true, false), (false, true), (false, false)] {
            let aria = aria_graph_kind_toggle(locale, "Reference", "4", active, can_toggle);
            assert!(
                aria.starts_with("Reference 4"),
                "WCAG Label in Name broken for {locale:?}: {aria}"
            );
        }
    }

    for i in 0..LOCALES.len() {
        for j in (i + 1)..LOCALES.len() {
            assert_ne!(single_notes.get(i), single_notes.get(j));
            assert_ne!(multi_notes.get(i), multi_notes.get(j));
        }
    }
}
