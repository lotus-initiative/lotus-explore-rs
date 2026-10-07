// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `error_presenter`, in their own file.

use super::*;

#[test]
fn both_lookup_notices_are_formatted_and_neither_is_dropped() {
    // `bacteria` is two true things at once: spelled differently from the
    // name it matched, and matched by more than one taxon. Formatting takes
    // a list precisely so neither can be the one that gets lost.
    let lines = format_taxon_warnings(
        Locale::En,
        &[
            LookupNotice::Standardized {
                original: "bacteria".into(),
                standardized: "Bacteria".into(),
            },
            LookupNotice::AmbiguousTaxon {
                chosen_name: "Bacteria".into(),
                chosen_qid: "Q10876".into(),
                candidates: vec!["Bacteria (Q10876)".into(), "Bacteria (Q4034791)".into()],
            },
        ],
    );

    assert_eq!(lines.len(), 2, "one line per notice: {lines:?}");
    let joined = lines.join("\n");
    assert!(
        joined.contains("bacteria") && joined.contains("Bacteria"),
        "{joined}"
    );
    assert!(
        joined.contains("Q10876") && joined.contains("Q4034791"),
        "{joined}"
    );
}

#[test]
fn a_compound_ambiguity_does_not_call_itself_a_taxon() {
    // The bug this pins: `c1ccccc1` resolves to several compounds, and the
    // notice said "Ambiguous taxon name" because both lookups shared one
    // variant and the presenter had the taxon wording hardcoded for it. The
    // reader was pointed at the field they did not type into.
    let lines = format_taxon_warnings(
        Locale::En,
        &[LookupNotice::AmbiguousCompound {
            chosen_name: "benzene".into(),
            chosen_qid: "Q2270".into(),
            candidates: vec![
                "benzene (Q2270)".into(),
                "deuterated benzene (Q1101314)".into(),
            ],
        }],
    );
    let joined = lines.join("\n");
    assert!(!joined.to_lowercase().contains("taxon"), "{joined}");
    assert!(joined.contains("compound"), "{joined}");
    assert!(joined.contains("Q2270"), "{joined}");
    assert!(joined.contains("deuterated benzene (Q1101314)"), "{joined}");
}

#[test]
fn each_ambiguity_names_its_own_kind_of_thing_in_every_locale() {
    // The sentence is not the English one in the other three, so asserting
    // English phrasing there would pass for the wrong reason. What has to hold
    // everywhere is that each names a taxon or a compound, and not the other.
    for locale in [Locale::En, Locale::Fr, Locale::De, Locale::It] {
        let taxon = format_taxon_warnings(
            locale,
            &[LookupNotice::AmbiguousTaxon {
                chosen_name: "Bacteria".into(),
                chosen_qid: "Q10876".into(),
                candidates: vec!["Bacteria (Q10876)".into()],
            }],
        )
        .join("\n");
        let compound = format_taxon_warnings(
            locale,
            &[LookupNotice::AmbiguousCompound {
                chosen_name: "benzene".into(),
                chosen_qid: "Q2270".into(),
                candidates: vec!["benzene (Q2270)".into()],
            }],
        )
        .join("\n");

        assert_ne!(taxon, compound, "{locale:?}: the two read the same");
        assert!(
            !taxon.to_lowercase().contains("compound"),
            "{locale:?}: {taxon}"
        );
        assert!(
            !compound.to_lowercase().contains("taxon"),
            "{locale:?}: {compound}"
        );
    }
}

#[test]
fn no_lookup_notices_formats_to_no_lines() {
    assert_eq!(format_taxon_warnings(Locale::En, &[]).len(), 0);
}

#[test]
fn compact_error_text_uses_exception_from_json() {
    let payload = r#"{"exception":"Upstream service returned HTTP 500","query":"SELECT ..."}"#;
    assert_eq!(
        compact_error_text(payload),
        "Upstream service returned HTTP 500"
    );
}

#[test]
fn transport_error_summary_truncates_long_network_message() {
    let long = "x".repeat(400);
    let summary = transport_error_summary(Locale::En, &RepositoryError::network(long));
    assert!(summary.chars().count() <= 221);
    assert!(summary.ends_with('…'));
}

#[test]
fn transport_error_summary_localizes_not_configured() {
    let en = transport_error_summary(Locale::En, &RepositoryError::NotConfigured);
    let fr = transport_error_summary(Locale::Fr, &RepositoryError::NotConfigured);
    // "configured" (EN) and "configurée" (FR) both share the ASCII stem "config".
    // `to_ascii_lowercase` does not strip accents, so checking for "configure" would
    // miss the French past-participle "configurée" whose 'é' is non-ASCII.
    assert!(en.to_ascii_lowercase().contains("config"));
    assert!(fr.to_ascii_lowercase().contains("config"));
}

#[test]
fn format_domain_error_renders_new_validation_faults() {
    let err = DomainError::Validation(ValidationFault::YearRangeInvalid);
    let rendered = format_domain_error(Locale::En, &err);
    assert!(rendered.contains("Year"));
}

#[test]
fn error_hint_for_http_4xx_is_bad_request_not_network() {
    let err = DomainError::transport(
        QueryStage::ResultsQuery,
        RepositoryError::Http {
            status: 400,
            body: "Invalid SPARQL query".to_string(),
        },
    );

    assert_eq!(
        error_hint_text(Locale::En, err.kind()),
        "The server rejected the request. Check your search parameters."
    );
}

#[test]
fn error_hint_for_transport_parse_uses_parse_hint() {
    let err = DomainError::transport(
        QueryStage::ResultsQuery,
        RepositoryError::parse("csv parse failed"),
    );

    assert_eq!(
        error_hint_text(Locale::En, err.kind()),
        t(Locale::En, TextKey::ErrorHintParse)
    );
}

#[test]
fn error_hint_for_not_configured_uses_configuration_hint() {
    let err = DomainError::transport(QueryStage::ResultsQuery, RepositoryError::NotConfigured);

    assert_eq!(
        error_hint_text(Locale::En, err.kind()),
        t(Locale::En, TextKey::ErrorHintConfiguration)
    );
}

#[test]
fn transport_error_summary_replaces_html_payloads_with_readable_text() {
    let summary = transport_error_summary(
        Locale::En,
        &RepositoryError::Http {
            status: 503,
            body: "<html><body>Service unavailable</body></html>".to_string(),
        },
    );

    assert_eq!(
        summary,
        "HTTP 503: Upstream service returned an HTML error page"
    );
}

#[test]
fn error_hint_for_rate_limit_uses_rate_limit_hint() {
    let err = DomainError::transport(
        QueryStage::ResultsQuery,
        RepositoryError::Http {
            status: 429,
            body: "<html><body>Too many requests</body></html>".to_string(),
        },
    );

    assert_eq!(
        error_hint_text(Locale::En, err.kind()),
        t(Locale::En, TextKey::ErrorHintRateLimit)
    );
}

#[test]
fn a_cancelled_query_is_advised_to_narrow_not_to_retry() {
    // The hint is the user-facing half of the change: every other failure in
    // this file ends by suggesting a retry, and for this one a retry is
    // precisely the wrong advice. The endpoint's budget was spent once
    // already; a second click spends it again.
    let err = DomainError::Transport {
        stage: QueryStage::ResultsQuery,
        source: RepositoryError::Http {
            status: 429,
            body: "Operation timed out. Last operation: Sort (internal order) on ?r".to_string(),
        },
    };

    assert_eq!(err.kind(), ErrorKind::QueryTooExpensive);
    let hint = error_hint_text(Locale::En, err.kind());
    assert_eq!(hint, t(Locale::En, TextKey::ErrorHintQueryTooExpensive));
    assert!(
        !hint.to_lowercase().contains("retry"),
        "the hint must not suggest a retry: {hint}"
    );
}

/// The regression: a translated frame with an English sentence inside it.
///
/// `lotus-query` writes one English sentence when a body ends in the notice
/// `QLever` appends to a `200` whose query ran out of time. It reached the user
/// through four layers as a bare `String`, every layer called it a generic parse
/// failure, and `format_parse_fault` interpolated it into a translated frame --
/// so a French or German reader saw their own language and then a paragraph of
/// English, with the wrong advice ("refine your query") for a result set that
/// was merely incomplete.
#[test]
fn the_endpoints_truncation_notice_is_reported_in_the_readers_language() {
    // The detail `lotus-query` produces, verbatim.
    let detail = "the endpoint stopped sending before the result was complete, so these \
                  rows are only part of the answer and are not being reported as if they \
                  were all of it (the endpoint said: Operation timed out); re-run the \
                  search, or narrow it";

    for locale in [Locale::En, Locale::Fr, Locale::De, Locale::It] {
        let rendered = super::format_domain_error(
            locale,
            &DomainError::Parse(ParseFault::ResultsCsv {
                details: detail.to_string(),
            }),
        );

        assert!(
            !rendered.contains("the endpoint stopped sending"),
            "{locale:?} still shows the raw English detail:\n{rendered}"
        );
        assert!(
            !rendered.contains("(the endpoint said:"),
            "{locale:?} still shows the raw reason clause:\n{rendered}"
        );
        assert!(
            !rendered.contains("could not parse the response"),
            "{locale:?} still reports it as a parse failure rather than a truncation:\n{rendered}"
        );
        // And it says the answer is partial, which is the actionable part.
        assert!(
            rendered.chars().count() > 40,
            "{locale:?} says too little to act on: {rendered}"
        );
    }

    // The four are actually different, so a missing translation cannot pass.
    let messages: Vec<String> = [Locale::En, Locale::Fr, Locale::De, Locale::It]
        .iter()
        .map(|locale| {
            super::format_domain_error(
                *locale,
                &DomainError::Parse(ParseFault::ResultsCsv {
                    details: detail.to_string(),
                }),
            )
        })
        .collect();
    for (i, a) in messages.iter().enumerate() {
        for b in messages.iter().skip(i + 1) {
            assert_ne!(a, b, "two locales render the same truncation message");
        }
    }
}

/// The reason is translated too, and it survives being read back out.
#[test]
fn each_reason_the_endpoint_can_give_has_its_own_sentence() {
    let with = |reason: &str| {
        format!(
            "the endpoint stopped sending before the result was complete, so these rows \
             are only part of the answer and are not being reported as if they were all \
             of it (the endpoint said: {reason}); re-run the search, or narrow it"
        )
    };

    let rendered: Vec<String> = ["Operation timed out", "Query was canceled", "Brand new"]
        .iter()
        .map(|reason| {
            super::format_domain_error(
                Locale::En,
                &DomainError::Parse(ParseFault::ResultsCsv {
                    details: with(reason),
                }),
            )
        })
        .collect();

    for (i, a) in rendered.iter().enumerate() {
        for b in rendered.iter().skip(i + 1) {
            assert_ne!(
                a, b,
                "two reasons render identically, so the reader cannot tell a query \
                 that timed out from one that was cancelled"
            );
        }
    }
    // An unrecognised reason is still reported -- the answer is still partial --
    // rather than dropped.
    assert!(
        rendered.iter().all(|message| !message.is_empty()),
        "an unknown reason must still produce a message"
    );
}
