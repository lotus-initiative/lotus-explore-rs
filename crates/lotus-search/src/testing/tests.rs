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
    assert_eq!(
        http.queries()[0],
        "http://example.org|PREFIX x: <http://x/>"
    );
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
