// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for the container health probe, in their own file.
//!
//! What can go wrong here is invisible until a container is failing to start or
//! a rolling deploy is quietly serving 500s: the probe reports healthy for a
//! server that is not, or unhealthy for one that is. Both are tested against a
//! real socket on a real port rather than a mock, because the whole of this
//! program is the socket.

// The panic lints keep library code from panicking on bad input. A test that
// fails on a bad fixture is reporting a failure, not panicking on input.
#![allow(clippy::expect_used, clippy::panic)]

use super::{probe, request};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

/// How many 10 ms polls `serve_once` will make before giving up on a connection.
///
/// Long enough that a loaded machine still accepts in time, short enough that a
/// test which never connects fails in about a second instead of hanging.
const ACCEPT_TIMEOUTS: usize = 200;

/// A listener on an ephemeral port, and the `host:port` to reach it on.
fn bind() -> (TcpListener, String, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("the bound address").port();
    (listener, "127.0.0.1".to_owned(), port)
}

/// Serve exactly one request with `response`, on a background thread.
///
/// One connection, not a loop: each test asserts on one answer, and a loop would
/// leave a thread blocked on a second accept that never comes.
///
/// The accept is bounded, because the alternative is a test that hangs rather
/// than fails. Every test here goes through `probe`, and a `probe` broken
/// enough to make no connection at all would leave the thread in `accept()`
/// until the runner gives up, reporting the mutant as a timeout instead of
/// caught.
fn serve_once(listener: TcpListener, response: &'static str) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("the listener can be polled instead of blocked on");
        let stream = (0..ACCEPT_TIMEOUTS)
            .find_map(|_| match listener.accept() {
                Ok((stream, _)) => Some(stream),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(10));
                    None
                }
                Err(e) => panic!("accept failed: {e}"),
            })
            .expect("a connection within the accept timeout");
        let mut stream = stream;
        // The accepted socket can inherit the listener's non-blocking flag, and
        // where it does, every read below returns `WouldBlock` rather than
        // waiting. Blocking it again is what makes the reads behave.
        stream
            .set_nonblocking(false)
            .expect("the accepted stream can be made blocking");
        // Read the request before answering, so the client is never writing into
        // a socket the server has already closed.
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("the request line");
        // Drain the headers; the request has one blank line ending them.
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
                break;
            }
        }
        stream.write_all(response.as_bytes()).expect("a write");
        stream.flush().expect("a flush");
        line
    })
}

/// 2xx and 3xx are both healthy, and 3xx is the case that decides it: the
/// server redirects `/` to `/search` on purpose, so a 3xx is the server
/// routing rather than the server failing, and treating it as unhealthy would
/// fail a container that is working exactly as designed.
#[test]
fn a_2xx_or_3xx_is_healthy() {
    for (response, path, expected) in [
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n",
            "/health",
            200,
        ),
        (
            "HTTP/1.1 302 Found\r\nLocation: /search\r\nContent-Length: 0\r\n\r\n",
            "/",
            302,
        ),
    ] {
        let (listener, host, port) = bind();
        let server = serve_once(listener, response);
        assert_eq!(
            request(&host, port, path).expect("a healthy answer"),
            expected
        );
        server.join().expect("the server thread");
    }
}
/// A 5xx is the server failing and a 4xx is the path being wrong; both are
/// unhealthy, and both errors must name the status so an operator reading the
/// log knows which one it was.
#[test]
fn a_4xx_or_5xx_is_unhealthy_and_names_the_status() {
    for (response, path, status) in [
        (
            "HTTP/1.1 500 Internal Server Error\r\n\r\n",
            "/health",
            "500",
        ),
        ("HTTP/1.1 404 Not Found\r\n\r\n", "/nope", "404"),
    ] {
        let (listener, host, port) = bind();
        let server = serve_once(listener, response);
        let err = request(&host, port, path).expect_err("a 4xx/5xx is not healthy");
        assert!(
            err.to_string().contains(status),
            "the error should name the status {status}, got: {err}"
        );
        server.join().expect("the server thread");
    }
}
#[test]
fn a_response_that_is_not_http_is_unhealthy() {
    // What a port that is not the server actually looks like: some other
    // protocol answering. Parsing the second token as a status is what makes
    // this an error rather than a nonsense number.
    let (listener, host, port) = bind();
    let server = serve_once(listener, "SSH-2.0-OpenSSH_9.6\r\n");
    let err = request(&host, port, "/health").expect_err("not HTTP is not healthy");
    assert!(
        err.to_string().contains("no HTTP status line"),
        "the error should say the response was not HTTP, got: {err}"
    );
    server.join().expect("the server thread");
}

#[test]
fn nothing_listening_is_unhealthy() {
    // Bind and drop, so the port is known to be free. A probe that cannot tell
    // "refused" from "healthy" is not a probe.
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("the bound address").port();
    drop(listener);
    assert!(
        request("127.0.0.1", port, "/health").is_err(),
        "a closed port must not be reported healthy"
    );
}

/// The exit status, which is the entire contract of a health check.
///
/// Docker reads only this: a probe that returns the wrong one is not a probe
/// that is slightly off, it is a container that stays in rotation while
/// serving 500s, or one that gets killed while serving 200s. Both directions
/// are asserted, and against a real socket, because that is what `main` does.
#[test]
fn the_exit_status_is_what_a_healthy_server_gets() {
    let (listener, host, port) = bind();
    let server = serve_once(listener, "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
    assert_eq!(
        probe(&host, port, "/health"),
        std::process::ExitCode::SUCCESS,
        "a 200 is healthy"
    );
    server.join().expect("the server thread");
}

/// The other direction, and the more damaging one.
#[test]
fn the_exit_status_is_not_what_a_failing_server_gets() {
    let (listener, host, port) = bind();
    let server = serve_once(listener, "HTTP/1.1 500 Oops\r\nContent-Length: 0\r\n\r\n");
    assert_eq!(
        probe(&host, port, "/health"),
        std::process::ExitCode::FAILURE,
        "a 500 is not healthy"
    );
    server.join().expect("the server thread");
}

/// Nothing listening at all.
///
/// The case a real deploy hits first: the server has not bound the port yet,
/// so every connection is refused. Reported healthy here would be the probe
/// claiming the container is serving traffic that cannot arrive.
#[test]
fn the_exit_status_is_failure_when_nothing_is_listening() {
    // Bind and immediately drop, so the port is known to be closed.
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        listener.local_addr().expect("the bound address").port()
    };
    assert_eq!(
        probe("127.0.0.1", port, "/health"),
        std::process::ExitCode::FAILURE,
        "a refused connection is not healthy"
    );
}

#[test]
fn a_host_that_does_not_resolve_is_an_error_not_a_panic() {
    // `.invalid` is reserved by RFC 2606 and never resolves, so this is a
    // resolution failure rather than a network one.
    let err = request("no-such-host.invalid", 8787, "/health").expect_err("no such host");
    assert!(
        !err.to_string().is_empty(),
        "the error should say something"
    );
}

#[test]
fn a_truncated_status_line_is_an_error() {
    // A server that accepts the connection and then closes without answering.
    // Reading zero bytes is the case where a parser that indexes the first line
    // would panic.
    let (listener, host, port) = bind();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().expect("a connection");
        drop(stream);
    });
    assert!(
        request(&host, port, "/health").is_err(),
        "a connection closed with no answer must not be healthy"
    );
    server.join().expect("the server thread");
}
