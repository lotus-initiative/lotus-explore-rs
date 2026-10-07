// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Recognising the endpoint's truncation notice, and reducing its reason.
//!
//! The two halves are pinned together deliberately. `NOTICE_CLAUSE` is a copy of
//! a clause of a sentence `lotus_query` writes in another crate, and if that
//! wording changes the detection silently stops working -- every truncation goes
//! back to being reported as an untranslated parse error, which is the bug this
//! exists to fix. The first test here fails in that case, on purpose.

#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use super::{NOTICE_CLAUSE, REASON_CLOSE, REASON_OPEN, Reason, truncation_reason};

/// The sentence `lotus_query::parse::stream::truncated_by_endpoint` writes.
fn detail(reason: &str) -> String {
    format!(
        "the endpoint stopped sending before the result was complete, so these rows \
         are only part of the answer and are not being reported as if they were all \
         of it (the endpoint said: {reason}); re-run the search, or narrow it"
    )
}

#[test]
fn the_clause_this_matches_on_is_the_one_lotus_query_writes() {
    // The cross-crate coupling, stated as a test. If `lotus-query` rewords the
    // sentence, this fails and says which constant to change -- which is the only
    // thing that makes a copied string safe to keep.
    let written = lotus_query_truncation_text();
    assert!(
        written.starts_with(NOTICE_CLAUSE),
        "`lotus-query` reworded its truncation sentence. `NOTICE_CLAUSE` here is a \
         copy of its opening clause and must be updated with it, or every \
         truncation goes back to being reported as an untranslated parse error.\nwritten: {written:?}\nclause:  {NOTICE_CLAUSE:?}"
    );
    // A prefix rather than equality, so the clause may stop short of the
    // sentence's first comma -- which is where it stops today.
    assert!(
        written
            .get(NOTICE_CLAUSE.len()..)
            .is_some_and(|rest| rest.starts_with(", so these rows")),
        "the clause must stop at a clause boundary, not mid-phrase: {written:?}"
    );
}

/// The truncation sentence as `lotus-query` compiles it.
///
/// A test reading a source file is unusual and this is the one place it is right:
/// `NOTICE_CLAUSE` is a copy of a string inside another crate's private code, and
/// the alternative is a second hand-maintained copy that nobody notices when the
/// first moves. If this test ever fails, `lotus-query` reworded the sentence and
/// this constant has to follow it.
fn lotus_query_truncation_text() -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/lotus-query/src/parse/stream.rs"
    );
    let source = std::fs::read_to_string(path).expect("the lotus-query source is readable");
    let at = source
        .find("\"the endpoint stopped sending")
        .expect("lotus-query still writes a truncation sentence");
    let literal = source
        .get(at + 1..)
        .expect("in bounds")
        .split_once('"')
        .map_or_else(String::new, |(body, _)| body.to_string());

    // A `\` followed by a newline and indentation is a source-level line
    // continuation; the compiled string has no break in it, which is what
    // `NOTICE_CLAUSE` is compared against.
    let mut out = String::new();
    let mut rest = literal.as_str();
    while let Some(mark) = rest.find('\\') {
        out.push_str(rest.get(..mark).expect("in bounds"));
        rest = rest
            .get(mark + 1..)
            .expect("in bounds")
            .trim_start_matches([' ', '\t'])
            .strip_prefix('\n')
            .expect("a continuation is a backslash, then a newline")
            .trim_start_matches([' ', '\t']);
    }
    out.push_str(rest);
    out
}

#[test]
fn the_reason_is_read_out_of_the_notice() {
    for reason in ["Operation timed out", "Query was canceled", "Something new"] {
        assert_eq!(
            truncation_reason(&detail(reason)),
            Some(reason),
            "the endpoint's own reason must survive: a timed-out query and a
             cancelled one are different problems"
        );
    }
}

#[test]
fn an_ordinary_parse_failure_is_not_a_truncation() {
    // The whole point of the function: every other parse failure must return
    // `None`, or an untranslatable sentence would be recognised as a truncation
    // and answered with the wrong advice.
    for other in [
        "",
        "the response ended in the middle of a row",
        "Variable ?s was not declared",
        "no taxon matched \"Rosa\"",
        REASON_OPEN,
        &format!("some other failure {REASON_OPEN} Operation timed out{REASON_CLOSE}"),
        &detail("Operation timed out").replace(REASON_OPEN, "nothing like it"),
    ] {
        assert_eq!(
            truncation_reason(other),
            None,
            "{other:?} is not the endpoint's truncation notice"
        );
    }
}

#[test]
fn the_two_known_reasons_are_reduced_and_anything_else_is_reported_as_unknown() {
    // Matched case-insensitively and without the trailing stop, because the
    // notice is QLever's and its punctuation is not a contract.
    assert_eq!(Reason::of("Operation timed out"), Reason::TimedOut);
    assert_eq!(Reason::of("operation timed out."), Reason::TimedOut);
    assert_eq!(Reason::of("  Operation Timed Out  "), Reason::TimedOut);
    assert_eq!(Reason::of("Query was canceled"), Reason::Cancelled);
    // QLever spells it with one `l`, and matching the list rather than
    // normalising the spelling means a service change shows up as `Other` -- which
    // is reported, not dropped.
    assert_eq!(Reason::of("query was cancelled."), Reason::Other);

    // An unrecognised reason is still a reason, so it is reported as not-given
    // rather than dropped or guessed at.
    assert_eq!(
        Reason::of("Something the service added last week"),
        Reason::Other
    );
    assert_eq!(Reason::of(""), Reason::Other);
}

#[test]
fn every_reason_and_locale_produce_a_message_with_nothing_left_english() {
    use crate::i18n::{Locale, err_truncated_by_endpoint};

    const LOCALES: [Locale; 4] = [Locale::En, Locale::Fr, Locale::De, Locale::It];

    // The regression this whole module exists for: a translated frame with an
    // English sentence inside it.
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
