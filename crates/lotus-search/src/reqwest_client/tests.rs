// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `reqwest_client`, in their own file.
//!
//! The adapter is tested against a real socket on loopback rather than a mock,
//! because the adapter *is* the socket: what can go wrong here is a status read
//! from the wrong place, a body handed back empty, or a byte count that does not
//! match what arrived, and only a real response can show any of them.

// A test that fails on a broken fixture is reporting a failure, not panicking on
// input, which is what the panic lints exist to prevent.
#![allow(clippy::expect_used)]

use super::*;
use std::time::Duration;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};

#[test]
fn a_read_that_failed_before_any_byte_is_still_a_connection_problem() {
    // Nothing was transferred, so sending the request again costs nothing
    // and might work. This is the retryable half.
    let err = ReadProgress::default().failed("connection reset".into());
    assert_eq!(err, FetchError::Network("connection reset".into()));
    assert!(err.is_retryable(), "a failed connection is worth retrying");
}

#[test]
fn a_read_that_failed_after_bytes_is_a_cut_short_result_set() {
    // This is the rule the timeout complaint actually turns on: the same
    // socket error, but with half a gigabyte already transferred, and so
    // not something to answer by starting over.
    let err = ReadProgress { bytes_read: 4096 }.failed("connection reset".into());
    assert_eq!(
        err,
        FetchError::Truncated {
            bytes_read: 4096,
            reason: TruncationReason::ClosedEarly,
        }
    );
    assert!(!err.is_retryable(), "a cut-short body is not retried");
}

#[test]
fn the_one_byte_threshold_is_a_real_boundary_and_not_a_range() {
    // Mutating `> 0` to `>= 0` would make every failure untouchable if it
    // were wrong in the other direction, and `>= 1` to `> 0` would make every
    // failure retryable. Both are single-character changes with opposite
    // effects, so both ends are pinned.
    assert!(
        ReadProgress { bytes_read: 0 }
            .failed("x".into())
            .is_retryable(),
        "nothing read is a connection problem"
    );
    assert!(
        ReadProgress { bytes_read: 1 }
            .failed("x".into())
            .is_truncated(),
        "one byte read is a body cut short"
    );
}

#[test]
fn every_chunk_that_arrives_is_counted_and_the_end_is_not() {
    let mut progress = ReadProgress::default();
    let first = progress.arrived(Some(ResponseBody::from_static(b"abc")));
    let second = progress.arrived(Some(ResponseBody::from_static(b"de")));
    let end = progress.arrived(None);

    assert_eq!(
        first.as_deref(),
        Some(&b"abc"[..]),
        "the chunk is handed on"
    );
    assert_eq!(second.as_deref(), Some(&b"de"[..]), "and so is the next");
    assert_eq!(end, None, "the end of the body stays the end");
    assert_eq!(progress.bytes_read, 5, "three bytes and then two");
    assert_eq!(
        progress.stalled(),
        FetchError::Truncated {
            bytes_read: 5,
            reason: TruncationReason::Stalled,
        },
        "a stall reports how far the body got"
    );
}

#[test]
fn the_client_names_itself_and_not_its_internals() {
    // `reqwest::Client`'s own `Debug` prints its whole configuration; this one
    // is a handle, and says so.
    let client = ReqwestClient::new().expect("a client builds without a network");
    assert_eq!(format!("{client:?}"), "ReqwestClient");
}

/// How long the server waits for the client to connect.
///
/// Long enough that a loaded machine still connects in time, short enough that
/// an adapter which never sends a request fails in seconds instead of hanging --
/// which a mutation run would report as a timeout rather than as caught.
const ACCEPT_WITHIN: Duration = Duration::from_secs(5);

/// Serve one request on loopback, answering with `parts` written one at a time.
///
/// The parts are separate writes with a pause between them, so a body can be made
/// to arrive in pieces, or to stop partway. The handle yields the request as it
/// was received, head and body.
async fn serve_once(parts: &'static [&'static str]) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free loopback port");
    let url = format!(
        "http://{}/sparql",
        listener.local_addr().expect("the bound address")
    );
    let server = tokio::spawn(async move {
        let (mut stream, _) = tokio::time::timeout(ACCEPT_WITHIN, listener.accept())
            .await
            .expect("a connection within the accept timeout")
            .expect("an accepted connection");
        let request = read_request(&mut stream).await;
        for part in parts {
            stream.write_all(part.as_bytes()).await.expect("a write");
            stream.flush().await.expect("a flush");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        request
    });
    (url, server)
}

/// Read one request, head and body, before answering it.
///
/// Answering first and closing would leave the request's body unread, and a
/// socket closed with unread data is reset rather than shut down -- which the
/// client sees as a failure the test did not ask for.
async fn read_request(stream: &mut TcpStream) -> String {
    let mut received = Vec::new();
    let mut buffer = [0_u8; 1024];
    while !is_complete(&received) {
        let n = stream.read(&mut buffer).await.expect("a read");
        if n == 0 {
            break;
        }
        received.extend(buffer.iter().take(n));
    }
    String::from_utf8_lossy(&received).into_owned()
}

/// Whether `received` holds a whole request: a head, and as much body as its
/// `content-length` declares.
fn is_complete(received: &[u8]) -> bool {
    let Some(head_end) = received.windows(4).position(|w| w == b"\r\n\r\n") else {
        return false;
    };
    let head = String::from_utf8_lossy(received.get(..head_end).unwrap_or_default());
    let length = head
        .to_ascii_lowercase()
        .lines()
        .find_map(|line| line.strip_prefix("content-length:")?.trim().parse().ok())
        .unwrap_or(0_usize);
    received.len() >= head_end + 4 + length
}

/// What a body amounted to, read to its end or to its failure.
#[derive(Debug, PartialEq)]
struct Drained {
    body: Vec<u8>,
    end: Result<(), FetchError>,
}

/// Read chunks from `next` until it ends, fails, or has not ended after a bound.
///
/// The bound is what keeps a reader that never says "end" from hanging the test:
/// it is reported as an outcome rather than waited out.
async fn drain(mut next: impl AsyncFnMut() -> Result<Option<ResponseBody>, FetchError>) -> Drained {
    let mut body = Vec::new();
    for _ in 0..16 {
        match next().await {
            Ok(Some(chunk)) => body.extend_from_slice(&chunk),
            Ok(None) => return Drained { body, end: Ok(()) },
            Err(e) => return Drained { body, end: Err(e) },
        }
    }
    Drained {
        body,
        end: Err(FetchError::Parse("no end of body after 16 chunks".into())),
    }
}

/// A body sent with chunked transfer encoding, in two writes.
const IN_TWO_PIECES: &[&str] = &[
    "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n",
    "2\r\nde\r\n0\r\n\r\n",
];

#[tokio::test]
async fn the_status_and_text_are_what_the_server_sent() {
    let (url, server) =
        serve_once(&["HTTP/1.1 418 I'm a teapot\r\nContent-Length: 5\r\n\r\nhello"]).await;
    let client = ReqwestClient::new().expect("a client");

    let response = client.get(&url, "text/csv").await.expect("a response");

    // An unusual status on purpose: a constant `200` would pass a `200` test.
    assert_eq!(HttpResponse::status(&response), 418);
    assert_eq!(HttpResponse::text(response).await, Ok("hello".to_owned()));
    let request = server.await.expect("the server finished");
    assert!(
        request.starts_with("GET /sparql HTTP/1.1\r\n"),
        "a GET to the path asked for: {request}"
    );
    assert!(
        request.contains("accept: text/csv\r\n"),
        "the accept header reaches the wire: {request}"
    );
}

#[tokio::test]
async fn the_bytes_are_what_the_server_sent() {
    let (url, server) = serve_once(&["HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello"]).await;
    let client = ReqwestClient::new().expect("a client");

    let response = client
        .post_json(&url, r#"{"q":1}"#.to_owned())
        .await
        .expect("a response");

    assert_eq!(
        HttpResponse::bytes(response).await,
        Ok(ResponseBody::from_static(b"hello"))
    );
    let request = server.await.expect("the server finished");
    assert!(
        request.contains("content-type: application/json\r\n"),
        "a JSON body says so: {request}"
    );
    assert!(
        request.ends_with("\r\n\r\n{\"q\":1}"),
        "and is sent as given: {request}"
    );
}

#[tokio::test]
async fn a_body_read_a_chunk_at_a_time_arrives_whole_and_then_ends() {
    let (url, server) = serve_once(IN_TWO_PIECES).await;
    let client = ReqwestClient::new().expect("a client");

    let mut response = client
        .post_form(
            &url,
            "text/csv",
            "query=ASK%7B%7D".to_owned(),
            &[("api-user-agent", "lotus-test".to_owned())],
        )
        .await
        .expect("a response");

    assert_eq!(
        drain(async || HttpResponse::chunk(&mut response).await).await,
        Drained {
            body: b"abcde".to_vec(),
            end: Ok(()),
        }
    );
    let request = server.await.expect("the server finished");
    assert!(
        request.contains("api-user-agent: lotus-test\r\n"),
        "a header the caller adds reaches the wire: {request}"
    );
    assert!(
        request.ends_with("\r\n\r\nquery=ASK%7B%7D"),
        "and the form is sent as given: {request}"
    );
}

#[tokio::test]
async fn a_streamed_body_arrives_whole_and_then_ends() {
    let (url, server) = serve_once(IN_TWO_PIECES).await;
    let client = ReqwestClient::new().expect("a client");

    let response = client
        .post(&url, "text/csv", "query=ASK%7B%7D".to_owned())
        .await
        .expect("a response");
    let mut chunks = response.into_chunks().expect("native responses stream");

    assert_eq!(
        drain(async || chunks.next_chunk().await).await,
        Drained {
            body: b"abcde".to_vec(),
            end: Ok(()),
        }
    );
    server.await.expect("the server finished");
}

/// A body that stops short of its declared length is reported as cut short, with
/// the count of what did arrive -- not as a connection failure, which would be
/// retried from the start.
#[tokio::test]
async fn a_body_that_stops_partway_says_how_far_it_got() {
    let (url, server) = serve_once(&["HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nabcd"]).await;
    let client = ReqwestClient::new().expect("a client");

    let response = client
        .post(&url, "text/csv", "query=ASK%7B%7D".to_owned())
        .await
        .expect("a response");
    let mut chunks = response.into_chunks().expect("native responses stream");
    server.await.expect("the server finished and closed");

    assert_eq!(
        drain(async || chunks.next_chunk().await).await,
        Drained {
            body: b"abcd".to_vec(),
            end: Err(FetchError::Truncated {
                bytes_read: 4,
                reason: TruncationReason::ClosedEarly,
            }),
        }
    );
}
