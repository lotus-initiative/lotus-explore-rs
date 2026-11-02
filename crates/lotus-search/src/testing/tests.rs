// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `testing`, in their own file.

use super::*;

#[tokio::test]
async fn the_script_is_consumed_in_order() {
    let http = Scripted::new(vec![(200, "one"), (200, "two")]);
    let first = http
        .post("http://example.org", "text/plain", "query=a".into())
        .await;
    let second = http
        .post("http://example.org", "text/plain", "query=b".into())
        .await;
    assert_eq!(first.expect("a reply").text().await.expect("text"), "one");
    assert_eq!(second.expect("a reply").text().await.expect("text"), "two");
}

#[tokio::test]
async fn a_zero_status_is_a_failed_connection_rather_than_a_rejection() {
    let http = Scripted::new(vec![(0, "")]);
    let err = http
        .post("http://example.org", "text/plain", "query=a".into())
        .await
        .expect_err("a status of 0 never arrives");
    assert!(
        matches!(err, FetchError::Network(_)),
        "the transport must tell them apart: {err:?}"
    );
}

#[test]
fn a_recorded_query_is_decoded_back_to_what_was_asked() {
    let http = Scripted::new(vec![]);
    http.record(
        "http://example.org",
        "query=PREFIX+x%3A+%3Chttp%3A%2F%2Fx%2F%3E",
    );
    assert_eq!(http.queries(), ["http://example.org|PREFIX x: <http://x/>"]);
    assert_eq!(http.endpoints(), ["http://example.org"]);
}

#[tokio::test]
async fn a_retried_request_is_counted_more_than_once() {
    // `call_count` includes retries, which is the point: a test that means
    // "one request" has to see a retry it did not ask for.
    let http = Scripted::new(vec![(200, "one"), (200, "two")]);
    let _ = http.post("http://example.org", "t", "query=a".into()).await;
    let _ = http.post("http://example.org", "t", "query=b".into()).await;
    assert_eq!(http.call_count(), 2);
}

#[tokio::test]
async fn every_body_is_recorded_exactly_as_it_was_sent() {
    // `queries` decodes; this is the undecoded view, so the encoding must survive.
    let http = Scripted::new(vec![(200, ""), (200, "")]);
    let _ = http
        .post("http://example.org", "t", "query=a+b&timeout=30s".into())
        .await;
    let _ = http
        .post_json("http://example.org", r#"{"q":1}"#.into())
        .await;
    assert_eq!(http.raw_bodies(), ["query=a+b&timeout=30s", r#"{"q":1}"#]);
}

#[tokio::test]
async fn every_call_has_its_header_names_and_only_their_names() {
    // One entry per call, including a call that sent none, so that entry `i` is
    // about the same request as `queries()[i]`.
    let http = Scripted::new(vec![(200, ""), (200, "")]);
    let _ = http
        .post_form(
            "http://example.org",
            "t",
            "query=a".into(),
            &[
                ("api-user-agent", "lotus".into()),
                ("Authorization", "Bearer secret".into()),
            ],
        )
        .await;
    let _ = http.get("http://example.org", "t").await;
    assert_eq!(
        http.header_names(),
        [vec!["api-user-agent", "Authorization"], vec![]]
    );
}

/// Read a scripted response with `chunk` to the end, bounded so that a reader
/// which never ends fails here instead of hanging.
async fn chunks_of(response: &mut ScriptedResponse) -> Vec<String> {
    let mut pieces = Vec::new();
    for _ in 0..8 {
        match response.chunk().await.expect("a scripted chunk") {
            Some(piece) => pieces.push(String::from_utf8_lossy(&piece).into_owned()),
            None => return pieces,
        }
    }
    pieces.push("<no end of body>".into());
    pieces
}

#[tokio::test]
async fn a_body_read_in_place_comes_in_the_scripted_pieces_and_then_ends() {
    let http = Scripted::with_chunk_size(vec![(200, "abcde")], 2);
    let mut response = http.get("http://example.org", "t").await.expect("a reply");
    assert_eq!(chunks_of(&mut response).await, ["ab", "cd", "e"]);
}

#[tokio::test]
async fn a_chunk_size_of_zero_hands_the_body_over_in_one_piece() {
    // Zero is "unchunked", not a step of zero: a step of zero never reaches the
    // end of the body, so every reader of it would spin.
    let http = Scripted::new(vec![(200, "abcde"), (200, "")]);
    let mut whole = http.get("http://example.org", "t").await.expect("a reply");
    assert_eq!(chunks_of(&mut whole).await, ["abcde"]);
    let mut empty = http.get("http://example.org", "t").await.expect("a reply");
    assert_eq!(
        chunks_of(&mut empty).await,
        Vec::<String>::new(),
        "an empty body ends at once"
    );
}

#[test]
fn every_kind_of_escape_is_decoded_and_everything_else_is_kept() {
    // `%` with no hex after it is not an escape, and must come through as typed
    // rather than swallowing the characters that follow it.
    assert_eq!(decode("query=a+b%2Bc%zz%4"), "a b+c%zz%4");
    assert_eq!(decode("no-prefix%20here"), "no-prefix here");
    assert_eq!(decode(""), "");
}
