// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `execute`, in their own file.

use super::*;

#[test]
fn an_error_body_becomes_one_readable_line() {
    let body = br#"{"exception":"Variable ?s was not declared","bindings":[]}"#;
    let compacted = compact(body);
    assert_eq!(compacted, "Variable ?s was not declared");
}

#[test]
fn a_gateway_pages_title_is_the_message_not_its_markup() {
    let body = b"<html>\n<head><title>502 Bad Gateway</title></head>\n</html>";
    assert_eq!(compact(body), "502 Bad Gateway");
}

#[test]
fn a_plain_text_error_becomes_one_line() {
    let body = b"\n  Something went wrong.  \n";
    assert_eq!(compact(body), "Something went wrong.");
}

#[test]
fn a_long_message_is_truncated_with_an_ellipsis() {
    let body = "x".repeat(500);
    let compacted = compact(body.as_bytes());
    assert!(compacted.chars().count() <= 241);
    assert!(compacted.ends_with('…'));
}

#[test]
fn a_query_is_form_encoded() {
    assert_eq!(urlencode("a b"), "a+b");
    assert_eq!(urlencode("?s"), "%3Fs");
    assert_eq!(urlencode("a=b&c"), "a%3Db%26c");
    assert_eq!(
        urlencode("SELECT-1._~"),
        "SELECT-1._~",
        "unreserved is untouched"
    );
}

#[test]
fn the_services_are_the_three_public_endpoints() {
    assert!(Service::Qlever.url().contains("qlever"));
    assert!(Service::Wdqs.url().contains("query.wikidata.org"));
    assert!(Service::Scholarly.url().contains("query-scholarly"));
}

#[test]
fn an_endpoint_reports_the_url_it_will_use() {
    let endpoint = Endpoint::new(Service::Qlever);
    assert_eq!(endpoint.url(), Service::Qlever.url());
    assert_eq!(endpoint.service(), Service::Qlever);
}

/// Each service names itself, its URL and its override, and the three are
/// distinct.
///
/// The URL decides where a query goes, the variable decides what a deployment
/// can point it at, and the name is what ends up in a result's provenance. A
/// constant in any one of them is silent -- the request still succeeds and the
/// answer is attributed to the wrong service -- so all three are pinned per
/// variant, and the set of each is checked for collisions.
#[test]
fn every_service_names_itself_and_its_endpoint() {
    let cases = [
        (
            Service::Qlever,
            QLEVER_WIKIDATA,
            "LOTUS_QLEVER_ENDPOINT",
            "QLever",
        ),
        (
            Service::Wdqs,
            WDQS_WIKIDATA,
            "LOTUS_WDQS_ENDPOINT",
            "Wikidata Query Service",
        ),
        (
            Service::Scholarly,
            WDQS_SCHOLARLY,
            "LOTUS_WDQS_SCHOLARLY_ENDPOINT",
            "Wikidata Query Service (scholarly subgraph)",
        ),
    ];

    for (service, url, variable, name) in cases {
        assert_eq!(service.url(), url, "{service:?} url");
        assert_eq!(
            service.variable(),
            variable,
            "{service:?} override variable"
        );
        assert_eq!(service.name(), name, "{service:?} provenance name");
        assert_eq!(
            service.to_string(),
            name,
            "{service:?} Display must be its provenance name, since that is what it is for"
        );
    }

    // Two services sharing a URL would send one service's query to the other,
    // and two sharing a variable would make an override apply to both.
    for (label, values) in [
        ("url", cases.map(|(_, url, _, _)| url)),
        ("variable", cases.map(|(_, _, variable, _)| variable)),
        ("name", cases.map(|(_, _, _, name)| name)),
    ] {
        let mut distinct = values.to_vec();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), cases.len(), "two services share a {label}");
    }
}

/// Two hundred-something is success, and the edges are where that lives.
///
/// `is_success` is a closed range on the low end and an open one on the high
/// end. Mutation testing turned `< 300` into `<= 300`, which nothing caught:
/// every fixture was a 200 or a 500, and a 300 -- which is neither success nor
/// an error the retry loop should act on -- never arrived.
#[test]
fn only_two_hundreds_are_success() {
    assert!(!is_success(199), "199 is not yet success");
    assert!(is_success(200), "200 is success");
    assert!(is_success(204), "204 is success with no body");
    assert!(is_success(299), "299 is the last success");
    assert!(!is_success(300), "300 is a redirect, not a success");
    assert!(!is_success(301), "301 is a redirect");
    assert!(!is_success(404), "404 is not success");
    assert!(!is_success(500), "500 is not success either");
}

/// The endpoint's own `exception` wins over the gateway's HTML.
///
/// A `QLever` error arrives as JSON with a useful message inside an HTML page.
/// Taking the HTML instead produces an error a reader cannot act on, which is
/// the whole reason this parses the body at all. The escaped-quote arm matters
/// because a message containing one would otherwise terminate the string early
/// and truncate the explanation mid-word.
#[test]
fn a_json_exception_is_read_out_of_an_html_page() {
    let body = br#"<html><body><h1>Error</h1><pre>{"exception": "Parser exception: unexpected ) at line 1", "code": "..."}</pre></body></html>"#;
    let line = compact(body);
    assert!(
        line.contains("unexpected )"),
        "the endpoint's own message must survive: {line}"
    );
    assert!(
        !line.contains("<html>"),
        "the HTML wrapper must not be what the reader is shown: {line}"
    );

    // An escaped quote inside the message must not end the string early.
    let escaped = br#"{"exception": "bad \"thing\" here", "code": "x"}"#;
    let line = compact(escaped);
    assert!(
        line.contains("thing"),
        "an escaped quote does not end the message: {line}"
    );

    // And a body with no exception at all still yields one usable line.
    let plain = compact(b"<html><body>502 Bad Gateway</body></html>");
    assert!(!plain.trim().is_empty(), "a body always yields a line");
}

/// Only an *unreachable* endpoint falls back to the other service.
///
/// This is the condition for trying `WDQS` after `QLever` fails, and getting it
/// backwards in either direction is expensive: fall back on a rejected query
/// sends a query the endpoint already refused to a second service and doubles
/// the wait, while never falling back turns a transient network failure into a
/// dead search. Mutation testing flipped this guard to both constants and the
/// suite caught neither.
#[test]
fn only_an_unreachable_endpoint_falls_back() {
    // A dropped connection is the case the fallback exists for.
    let dropped = FetchError::Network("connection reset".into());
    assert!(
        dropped.is_endpoint_unavailable(),
        "an unreachable endpoint is exactly when to try the other service"
    );

    // A gateway error is treated as unreachable, because the endpoint behind
    // it is what is being replaced.
    let gateway = FetchError::Http {
        status: 502,
        message: "bad gateway".into(),
    };
    assert!(
        gateway.is_endpoint_unavailable(),
        "502 means the endpoint behind the gateway is the problem"
    );

    // A rejected query is not an outage. The endpoint understood the query and
    // said no; the other service will say the same thing.
    let rejected = FetchError::Http {
        status: 400,
        message: "syntax error".into(),
    };
    assert!(
        !rejected.is_endpoint_unavailable(),
        "a rejected query must not be retried against a second service"
    );

    // Neither must a cancellation. QLever's 429 means this query outran its
    // time limit; WDQS would run the same expensive query a second time, on
    // a second endpoint, and time out there too.
    let cancelled = FetchError::TimedOut {
        budget: Some("30s".into()),
        message: "Operation timed out".into(),
    };
    assert!(
        !cancelled.is_endpoint_unavailable(),
        "a query that ran out of time is not an outage"
    );

    // And a malformed body is our problem, not the endpoint's.
    let malformed = FetchError::Parse("not csv".into());
    assert!(
        !malformed.is_endpoint_unavailable(),
        "a parse failure says the endpoint answered"
    );
}

// Pure, and until now reached only through `query_budget`, which reads an environment
// variable and so was not observable from a unit test: the workspace forbids
// `unsafe_code` and `std::env::set_var` is `unsafe` on this toolchain, leaving the
// budget arithmetic untested.
//
// Each case below is a spelling an operator can legitimately write in
// `LOTUS_QLEVER_TIMEOUT`. A silently misread budget is a query running for the wrong
// length of time, which nobody notices.

#[test]
fn a_budget_in_milliseconds_is_taken_as_written() {
    assert_eq!(clamp_budget("250ms").as_deref(), Some("250ms"));
}

#[test]
fn a_budget_in_seconds_becomes_milliseconds() {
    assert_eq!(clamp_budget("5s").as_deref(), Some("5000ms"));
}

#[test]
fn the_long_spelling_of_seconds_is_understood() {
    assert_eq!(clamp_budget("5sec").as_deref(), Some("5000ms"));
}

#[test]
fn a_budget_with_no_unit_is_seconds() {
    assert_eq!(clamp_budget("5").as_deref(), Some("5000ms"));
}

#[test]
fn a_budget_in_minutes_becomes_milliseconds() {
    assert_eq!(clamp_budget("1min").as_deref(), Some("30000ms"));
}

#[test]
fn whitespace_between_the_number_and_the_unit_is_tolerated() {
    assert_eq!(clamp_budget("5 s").as_deref(), Some("5000ms"));
}

#[test]
fn a_budget_over_the_ceiling_is_clamped_rather_than_refused() {
    assert_eq!(
        clamp_budget("10min").as_deref(),
        Some("30000ms"),
        "the public instance refuses more than 30s, so an over-large local \
         value must fail as a local clamp and not as a 403 from the endpoint"
    );
}

#[test]
fn the_ceiling_is_the_one_the_endpoint_enforces() {
    assert_eq!(clamp_budget("30s").as_deref(), Some("30000ms"));
}

#[test]
fn a_unit_that_is_not_a_unit_is_refused() {
    assert_eq!(
        clamp_budget("5h"),
        None,
        "hours are not a spelling this crate emits, and guessing would send \
         a budget nobody asked for"
    );
}

#[test]
fn text_that_is_not_a_duration_is_refused() {
    assert_eq!(clamp_budget("soon"), None);
}

#[test]
fn an_empty_value_is_refused() {
    assert_eq!(
        clamp_budget(""),
        None,
        "no digits means no amount, so the caller's default stands"
    );
}

#[test]
fn a_duration_that_would_overflow_is_refused_rather_than_wrapping() {
    // `checked_mul` is what makes this `None` instead of a small budget
    // nobody asked for, and a wrapped small budget would be accepted by the
    // ceiling check and sent to the endpoint.
    assert_eq!(clamp_budget("18446744073709551615min"), None);
    assert_eq!(clamp_budget("99999999999999999999999"), None);
}

#[test]
fn the_ceiling_is_read_from_the_endpoint_constant() {
    // If `MAX_QUERY_BUDGET` ever stops ending in `s`, the fallback below
    // takes over and the ceiling silently becomes 30s by coincidence. That
    // is the kind of coincidence worth a test.
    let parsed = QLever::MAX_QUERY_BUDGET
        .strip_suffix('s')
        .and_then(|seconds: &str| seconds.parse::<u64>().ok());
    assert_eq!(parsed, Some(30));
}

// The token half reads `LOTUS_QLEVER_TOKEN`, so only the half that does not is reachable
// here, pinned because a client that stopped identifying itself is a failure no status
// code reports.

#[test]
fn every_request_identifies_the_client_first() {
    let headers = request_headers();
    assert_eq!(
        headers
            .first()
            .map(|&(name, ref value)| (name, value.as_str())),
        Some(("api-user-agent", QLever::CLIENT_ID)),
        "the user agent is unconditional and comes first, so a reader of the \
         headers can rely on it being there"
    );
    assert!(
        headers.len() <= 2,
        "the only other header is the optional token, so there are at most two: {headers:?}"
    );
}
/// A non-ASCII message survives being read out of a JSON exception body.
///
/// `json_exception` finds the closing quote by walking the string and advancing a
/// byte offset, and the offset has to advance by the character's *width*: `é` is
/// two bytes, so an offset advanced by one byte per character lands in the middle
/// of it. The result of getting that wrong is not a crash but a message sliced at
/// the wrong place, which reads as an endpoint that garbled its own error -- and
/// every existing message test used ASCII, where the two are the same number.
#[test]
fn a_non_ascii_message_survives_being_read_out_of_a_json_exception() {
    // The last character before the closing quote is the one that matters, and it
    // is a multi-byte character here. `end.max(i)` absorbs any difference in the
    // offset for every character *except* the last, so a message ending in ASCII
    // cannot tell a correct byte offset from a wrong one -- which is why every
    // message test that used ASCII passed against a broken offset too.
    let message = "Compoundulo \u{2014} 12 mg, \u{2265} 99 % pur\u{e9}";
    let body = format!(r#"{{"exception":"{message}","bindings":[]}}"#);

    let compacted = compact(body.as_bytes());

    assert_eq!(
        compacted, message,
        "the endpoint's own words are what the user should read, unaltered"
    );
}

/// A reference lookup: `wdqs_fallback` routes these to the scholarly subgraph
/// rather than the main endpoint, because `P356` is slow there and this is the
/// one query that only needs `P356`.
const REFERENCE_LOOKUP: &str = "SELECT ?ref WHERE { ?ref wdt:P356 \"10.1000/nope\" . }";

#[test]
fn a_reference_lookup_falls_back_to_the_scholarly_subgraph_and_not_the_main_endpoint() {
    // The routing is the whole point of `wdqs_fallback`, and the Scholarly arm
    // had no test: a mutant that sent every fallback to the main endpoint would
    // still pass, because the main endpoint answers reference lookups too --
    // just slowly, which is the complaint the routing exists to answer.
    assert!(lotus_query::is_reference_lookup(REFERENCE_LOOKUP));
    let (service, _) = lotus_query::wdqs_fallback(REFERENCE_LOOKUP);
    assert!(
        matches!(service, lotus_query::FallbackService::Scholarly),
        "a reference lookup must route to the scholarly subgraph, got {service:?}"
    );

    // And a query that is not a reference lookup goes the other way.
    let (service, _) = lotus_query::wdqs_fallback(
        "SELECT ?compound WHERE { ?compound wdt:P31 wd:Q11365 . OPTIONAL { ?x ?y ?z } }",
    );
    assert!(
        matches!(service, lotus_query::FallbackService::Main),
        "an ordinary query must route to the main endpoint, got {service:?}"
    );
}

#[test]
fn an_exception_with_no_closing_quote_falls_through_to_the_next_reader() {
    // `json_exception` returns `None` when the value never closes -- a truncated
    // response, or an endpoint that cut the connection mid-body. `compact` then
    // falls through to the `<title>` reader and then to the first non-empty
    // line, so a truncated exception still produces one readable line rather
    // than no message at all.
    assert_eq!(
        json_exception(r#"{"exception":"Variable ?s was not decl"#),
        None,
        "an unterminated exception must not be reported as one"
    );
    let compacted = compact(br#"{"exception":"truncated mid-body"#);
    assert!(
        !compacted.is_empty(),
        "a truncated exception must still yield a readable line"
    );
    assert!(
        compacted.contains("truncated mid-body"),
        "the fallback reader dropped the message: {compacted:?}"
    );
}

#[test]
fn an_endpoint_unavailable_is_exactly_network_or_502() {
    // This predicate decides whether the query is retried against a second
    // endpoint. Anything broader and a rejected query runs twice on two public
    // endpoints; anything narrower and a real outage is not survived at all.
    assert!(FetchError::Network("connection refused".into()).is_endpoint_unavailable());
    assert!(
        FetchError::Http {
            status: 502,
            message: "Bad Gateway".into()
        }
        .is_endpoint_unavailable()
    );

    for not_unavailable in [
        // A 429 from QLever means the query ran out of time, not that the
        // endpoint is down. Falling back would spend WDQS's whole budget too.
        FetchError::Http {
            status: 429,
            message: "timeout".into(),
        },
        FetchError::Http {
            status: 400,
            message: "syntax error".into(),
        },
        FetchError::Http {
            status: 503,
            message: "maintenance".into(),
        },
        FetchError::TimedOut {
            budget: Some("60s".into()),
            message: "cancelled".into(),
        },
        FetchError::Parse("bad csv".into()),
    ] {
        assert!(
            !not_unavailable.is_endpoint_unavailable(),
            "{not_unavailable:?} must not trigger a fallback to a second endpoint"
        );
    }
}
