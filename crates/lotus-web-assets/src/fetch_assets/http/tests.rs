// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Tests for the shared fetch plumbing.
//!
//! The panic lints keep library code from panicking on bad input. A test that
//! fails on a bad fixture is reporting, not panicking.
#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing
)]

use serde_json::Value;
use std::fs;
use std::path::Path;

use super::*;
use crate::test_support::{
    MockServer, client, commit_id, err, http_ok, http_redirect_to_self, http_status, temp_dir,
};

#[test]
fn an_unset_setting_is_its_default() {
    // `setting_or` covers the filtering; this covers the lookup itself, which is
    // the half that would silently return an empty value.
    assert_eq!(
        setting("LOTUS_FETCH_ASSETS_NOT_SET_9f3a2b", "fallback"),
        "fallback"
    );
}

#[test]
fn a_blank_setting_counts_as_unset() {
    assert_eq!(setting_or(None, "fallback"), "fallback");
    for blank in ["", " ", "\t", "  \n ", "\r\n"] {
        assert_eq!(
            setting_or(Some(blank.to_owned()), "fallback"),
            "fallback",
            "{blank:?} is not a value, it is an empty export"
        );
    }
    assert_eq!(setting_or(Some("real".to_owned()), "fallback"), "real");
    assert_eq!(
        setting_or(Some(" padded ".to_owned()), "fallback"),
        " padded "
    );
}

#[test]
fn json_is_parsed_and_a_500_is_not() {
    // The mock answers one response per connection, and a 5xx is retried, so the
    // server has to be given the response once per attempt. This is the shape a
    // flaky CDN produces, and the point is that the fetch still reports the 500
    // rather than succeeding on an empty answer.
    let mut responses = vec![http_ok(r#"{"version":"1"}"#)];
    responses.extend((0..3).map(|_| http_status(500, "Server Error")));
    let server = MockServer::start(responses);
    let client = client(3000);
    assert_eq!(
        read_json(&client, &server.url("/a")).unwrap_or_default()["version"],
        Value::from("1")
    );
    let message = err(read_json(&client, &server.url("/b")));
    assert!(message.contains("500"), "got {message}");
}

#[test]
fn a_rate_limit_is_retried_but_a_not_found_is_not() {
    // 429 is the one 4xx that clears on its own, so it is retried; 404 is the
    // URL being wrong and repeating it only triples the wait. Both are 4xx, so
    // the distinction has to be the status rather than the class alone.
    for (code, reason, retried) in [
        (429u16, "Too Many Requests", true),
        (404, "Not Found", false),
    ] {
        let mut responses = vec![http_status(code, reason)];
        if retried {
            responses.push(http_ok(r#"{"version":"late"}"#));
        }
        let server = MockServer::start(responses);
        let url = server.url("/limited");

        let outcome = read_json(&client(3000), &url);

        if retried {
            assert_eq!(
                outcome.unwrap_or_default()["version"],
                Value::from("late"),
                "{code} is retried"
            );
        } else {
            assert!(err(outcome).contains("404"), "{code} is not retried");
        }
    }
}

#[test]
fn an_unreachable_host_is_attried_the_configured_number_of_times() {
    // Counting the attempts is the whole point. A guard that gave up on the
    // first attempt, or one that never gave up at all, produces the same error
    // message as the correct three -- so the count has to be observed rather
    // than inferred from the failure.
    //
    // Each request to a port nothing is listening on is refused before the mock
    // server sees anything, so this cannot be counted there. The doubling of
    // the waits is the observable proxy: three attempts sleep twice, one sleeps
    // not at all.
    let started = std::time::Instant::now();
    let outcome = read_json(&client(300), "http://127.0.0.1:1/nothing-here");
    let elapsed = started.elapsed();

    assert!(outcome.is_err(), "the host is not there to answer");
    assert!(
        elapsed >= BACKOFF * 2,
        "three attempts sleep at least twice: {:?} after {:?}",
        BACKOFF * 2,
        elapsed
    );
}

#[test]
fn an_unreachable_host_gives_up_after_the_configured_attempts() {
    // A port nothing is listening on refuses the connection, so every attempt
    // fails at the transport layer rather than with a status. The point is that
    // it stops: without the last-attempt guard this loops forever, and the error
    // a caller sees has to be the transport one rather than a truncated status.
    let message = err(read_json(&client(300), "http://127.0.0.1:1/nothing-here"));
    assert!(
        message.starts_with("could not reach"),
        "the transport failure is reported as such, got: {message}"
    );
}

#[test]
fn the_wait_between_attempts_grows() {
    // It has to grow: the point of waiting is to give a rate limit or an
    // overloaded CDN room to recover, and a wait that shrank or stayed flat
    // would hammer the endpoint hardest exactly when it is least able to answer.
    let waits: Vec<_> = (1..ATTEMPTS).map(backoff_after).collect();
    assert_eq!(waits.len(), 2, "three attempts means two waits");
    assert!(
        waits[1] > waits[0],
        "the second wait is longer than the first: {waits:?}"
    );
    assert_eq!(waits[0] * 2, waits[1], "and it doubles: {waits:?}");
    assert_eq!(backoff_after(1), BACKOFF, "the first wait is the base");
}

#[test]
fn a_4xx_is_not_retried() {
    // A 404 is the URL, not the server: repeating it only triples the wait
    // before the same answer. The mock queues one response, so a second attempt
    // would get a refused connection and the error would name that instead.
    let server = MockServer::start(vec![http_status(404, "Not Found")]);
    let message = err(read_json(&client(3000), &server.url("/gone")));
    assert!(message.contains("404"), "got {message}");
}

#[test]
fn a_transient_failure_is_retried_and_then_succeeds() {
    // The case that motivated the retry: one blip, and the fetch works. Without
    // it a CDN hiccup fails a 115 MB build.
    let server = MockServer::start(vec![
        http_status(503, "Service Unavailable"),
        http_ok(r#"{"version":"2"}"#),
    ]);
    assert_eq!(
        read_json(&client(3000), &server.url("/flaky")).unwrap_or_default()["version"],
        Value::from("2"),
        "the second attempt succeeds"
    );
}

#[test]
fn a_commit_id_is_forty_hex_characters_and_nothing_else() {
    assert!(is_commit_id(&commit_id('a')));
    assert!(is_commit_id(&commit_id('F')));
    assert!(
        !is_commit_id(&commit_id('a')[..39]),
        "one short is a branch"
    );
    assert!(
        !is_commit_id(&format!("{}a", commit_id('a'))),
        "one long is not an id"
    );
    assert!(
        !is_commit_id(&commit_id('z')),
        "hexadecimal only: a ref with a non-hex character is not a commit"
    );
    assert!(!is_commit_id("main"));
    assert!(!is_commit_id(""));
}

#[test]
fn a_commit_is_read_out_of_the_feed() {
    let body = format!("<entry>Grit::Commit/{}\n</entry>", commit_id('0'));
    assert_eq!(github_commit(&body).unwrap_or_default(), commit_id('0'));
}

#[test]
fn a_feed_with_no_commit_id_yields_nothing() {
    assert!(github_commit("<entry>nothing here</entry>").is_none());
    assert!(github_commit(&format!("Grit::Commit/{}", &commit_id('a')[..39])).is_none());
    assert!(
        github_commit(&format!("Grit::Commit/{}", commit_id('z'))).is_none(),
        "a non-hex id is not a commit"
    );
}

#[test]
fn extracts_github_commit_from_atom_feed() {
    let body = "<id>tag:github.com,2008:Grit::Commit/0123456789abcdef0123456789abcdef01234567</id>";
    assert_eq!(
        github_commit(body).as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567")
    );
    assert!(github_commit("<id>not-a-commit</id>").is_none());
}

#[test]
fn a_pinned_ref_is_taken_as_given() {
    let server = MockServer::start(vec![]);
    let pinned = commit_id('0');
    assert_eq!(
        resolve_github_ref(&client(300), &server.url(""), "WDscholia/scholia", &pinned)
            .unwrap_or_default(),
        pinned,
        "a full commit id needs no feed lookup"
    );
}

#[test]
fn a_branch_ref_resolves_through_the_feed() {
    let server = MockServer::start(vec![http_ok(&format!(
        "<entry>Grit::Commit/{}\n</entry>",
        commit_id('0')
    ))]);
    assert_eq!(
        resolve_github_ref(&client(3000), &server.url(""), "WDscholia/scholia", "main")
            .unwrap_or_default(),
        commit_id('0')
    );
    let request = server.next_request();
    assert!(
        request.contains("/WDscholia/scholia/commits/main.atom"),
        "got {request}"
    );
}

#[test]
fn a_ref_the_feed_does_not_resolve_is_an_error() {
    let server = MockServer::start(vec![http_ok("<entry>no id here</entry>")]);
    let message = err(resolve_github_ref(
        &client(3000),
        &server.url(""),
        "WDscholia/scholia",
        "gone",
    ));
    assert!(message.contains("no commit id"), "got {message}");
}

#[test]
fn a_ref_lookup_that_404s_is_an_error() {
    let server = MockServer::start(vec![http_status(404, "Not Found")]);
    let message = err(resolve_github_ref(
        &client(3000),
        &server.url(""),
        "WDscholia/scholia",
        "gone",
    ));
    assert!(message.contains("404"), "got {message}");
}

#[test]
fn a_pinned_version_never_asks_the_network() {
    let server = MockServer::start(vec![]);
    // The server has no responses queued, so any request would hang and then
    // fail; the assertion is that resolution succeeds without one.
    assert_eq!(
        resolve_tagged_version(&client(300), &server.url("/latest"), "v2.3.4").unwrap_or_default(),
        "2.3.4",
        "the leading v is stripped so versions compare equal"
    );
    assert_eq!(
        resolve_metadata_version(&client(300), &server.url("/meta"), "2026.3.6")
            .unwrap_or_default(),
        "2026.3.6",
        "a pinned metadata version needs no request either"
    );
}

#[test]
fn the_word_latest_resolves_through_the_redirect() {
    // The tag is read off the URL the redirect *resolved to*, so the mock
    // redirects to itself and the follow-up is what carries `/tag/`.
    let server = MockServer::start(vec![http_redirect_to_self("/tag/v9.9.9/"), http_ok("")]);
    let resolved = resolve_tagged_version(&client(3000), &server.url("/releases/latest"), "latest");
    assert!(
        matches!(resolved, Ok(ref v) if v == "9.9.9"),
        "the tag comes off the resolved URL, got {resolved:?}"
    );
    assert!(
        server.next_request().contains("/releases/latest"),
        "the latest URL is what was asked for"
    );
}

#[test]
fn a_latest_lookup_that_404s_is_an_error() {
    let server = MockServer::start(vec![http_status(404, "Not Found")]);
    let message = err(resolve_tagged_version(
        &client(3000),
        &server.url("/releases/latest"),
        "latest",
    ));
    assert!(message.contains("404"), "a 404 names itself, got {message}");
}

#[test]
fn a_latest_lookup_with_no_tag_in_the_url_is_an_error() {
    // Redirects to the repository root, not to a release: the version cannot be
    // read, and guessing one would vendor the wrong bundle.
    let server = MockServer::start(vec![
        http_redirect_to_self("/WDscholia/ketcher"),
        http_ok(""),
    ]);
    let message = err(resolve_tagged_version(
        &client(3000),
        &server.url("/releases/latest"),
        "latest",
    ));
    assert!(
        message.contains("did not resolve to a release tag"),
        "got {message}"
    );
}

#[test]
fn the_latest_metadata_version_comes_from_the_document() {
    let server = MockServer::start(vec![http_ok(r#"{"version":"2026.9.9"}"#)]);
    assert_eq!(
        resolve_metadata_version(&client(3000), &server.url("/meta"), "latest").unwrap_or_default(),
        "2026.9.9"
    );
    let _ = server.next_request();
}

#[test]
fn metadata_with_no_version_is_an_error() {
    let server = MockServer::start(vec![http_ok(r#"{"name":"@rdkit/rdkit"}"#)]);
    let message = err(resolve_metadata_version(
        &client(3000),
        &server.url("/meta"),
        "latest",
    ));
    assert!(message.contains("no version"), "got {message}");
}

#[test]
fn a_file_is_written_and_an_error_page_is_not() {
    let dir = temp_dir("fetch");
    let server = MockServer::start(vec![
        http_ok("contents"),
        http_status(403, "Forbidden"),
        http_ok(""),
    ]);
    let c = client(3000);

    let good = dir.join("nested/good.txt");
    fetch_file(&c, &server.url("/a"), &good).unwrap_or_default();
    assert_eq!(fs::read_to_string(&good).unwrap_or_default(), "contents");
    assert!(
        good.parent().is_some_and(Path::exists),
        "the parent is created"
    );

    let denied = dir.join("denied.txt");
    let message = err(fetch_file(&c, &server.url("/b"), &denied));
    assert!(message.contains("403"), "got {message}");
    assert!(
        !denied.exists(),
        "an error page must not be written as the asset"
    );

    let empty = dir.join("empty.txt");
    let error = err(fetch_file(&c, &server.url("/c"), &empty));
    assert!(error.contains("empty response"), "got {error}");
    assert!(!empty.exists(), "an empty body is not an asset");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_destination_that_cannot_be_written_is_an_error_not_a_silent_skip() {
    // The asset is fetched successfully, so the only thing that can fail is
    // writing it. Returning success here would leave the tree looking complete.
    let server = MockServer::start(vec![http_ok("contents")]);
    let blocker = temp_dir("fetch-blocked");
    let as_file = blocker.join("not-a-directory");
    fs::write(&as_file, "in the way").unwrap_or_else(|e| panic!("{e}"));

    let message = err(fetch_file(
        &client(3000),
        &server.url("/a"),
        &as_file.join("child.txt"),
    ));

    assert!(
        !message.is_empty(),
        "the failure is reported rather than swallowed"
    );
    let _ = fs::remove_dir_all(&blocker);
}
