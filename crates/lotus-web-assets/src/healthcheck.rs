// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! A health probe for the container image, and nothing else.
//!
//! The runtime image is `distroless/cc`: no shell, no HTTP client, so the usual
//! `CMD wget --spider ...` cannot run in it. `HEALTHCHECK` takes a binary and
//! arguments, so what it needs is a binary that fetches a URL and exits non-zero
//! on failure.
//!
//! The alternative was dropping `HEALTHCHECK`, leaving a container that has
//! stopped serving indistinguishable from one that is starting, or putting a
//! shell back in the image for the check to use.
//!
//! No dependencies of its own: compiled to musl and copied into the image, so
//! every dependency would be one the runtime image carries to answer a single
//! request. `std::net::TcpStream` plus a hand-written request is all HTTP/1.0
//! needs for a `GET`.
//!
//! The crate's other bins share one dependency set and this one touches none of
//! it, so `unused_crate_dependencies` (denied workspace-wide) is satisfied by
//! naming them as `_`. Same note as in `inject_wasm_preload.rs`.
use reqwest as _;
use serde_json as _;
use zip as _;

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::ExitCode;
use std::time::Duration;

/// How long to wait for the whole exchange.
///
/// Short on purpose: a check that takes 30 seconds to report a dead server
/// outlives the orchestrator's patience, and the container is killed by then for
/// an unrelated reason.
const TIMEOUT: Duration = Duration::from_secs(3);

/// `GET <path> HTTP/1.0` against `host:port`.
///
/// HTTP/1.0 rather than 1.1 so the server closes the connection when it is
/// done and this can simply read to end-of-stream. HTTP/1.1 would need
/// `Content-Length` handling to know when to stop, and this reads a response it
/// only needs the status line of.
///
/// # Errors
/// Returns an error if the address does not resolve, the connection fails or
/// times out, or the peer does not answer with an HTTP status line in the 2xx
/// or 3xx range.
pub fn request(host: &str, port: u16, path: &str) -> std::io::Result<u16> {
    let addr = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::other("host resolved to no addresses"))?;
    let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;

    write!(
        stream,
        "GET {path} HTTP/1.0\r\nHost: {host}\r\nUser-Agent: lotus-healthcheck\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()?;

    // The status line is the first line, and the headers before the body are
    // short, so 8 KiB is far more than the first line needs and far less than
    // a response body would be. The body is not read: only the status is
    // asserted on, and reading the rest of an export response to look at
    // something already known would be the slow way to learn nothing.
    let mut buf = [0_u8; 8192];
    let n = stream.read(&mut buf)?;

    // `HTTP/1.1 200 OK` -> 200. Anything that is not a status line is a
    // connection that answered with something other than HTTP, which for the
    // purpose of "is the server serving" is a failure.
    // `n` is what `read` reported, so it is in range; `.get` rather than a
    // slice because a zero-length read is a real case here (a peer that closes
    // without answering) and must be an error, not a panic.
    let head = buf
        .get(..n)
        .and_then(|b| std::str::from_utf8(b).ok())
        .unwrap_or_default();
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| std::io::Error::other("no HTTP status line in the response"))?;

    // 2xx and 3xx both mean the server is up and routing. A 3xx is a redirect
    // to somewhere, not a failure to answer, and treating it as unhealthy would
    // fail a container whose server is redirecting `/` to `/search` on purpose.
    if (200..400).contains(&status) {
        Ok(status)
    } else {
        Err(std::io::Error::other(format!("server answered {status}")))
    }
}

/// `lotus-healthcheck [host] [port] [path]`.
///
/// The defaults are the container's own configuration, so the `HEALTHCHECK`
/// line in the Dockerfile needs no arguments and cannot drift from what the
/// server was told to listen on.
fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let host = args.next().unwrap_or_else(|| "127.0.0.1".to_owned());
    let port: u16 = args.next().and_then(|p| p.parse().ok()).unwrap_or(8787);
    let path = args.next().unwrap_or_else(|| "/health".to_owned());

    probe(&host, port, &path)
}

/// The whole probe, minus `main`.
///
/// Split out so the mapping from outcome to exit status is a function returning a
/// value rather than a branch buried in `main`. `main` itself is only reachable by
/// running the binary and reading `$?`, which is the one thing a unit test cannot
/// do -- and a health check that reports healthy for a server that is not is the
/// failure mode that silently takes a container out of a load balancer.
fn probe(host: &str, port: u16, path: &str) -> ExitCode {
    match request(host, port, path) {
        Ok(status) => {
            println!("ok: {host}:{port}{path} -> {status}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            // On stderr, because a health check's stdout is often collected
            // verbatim and an error message in it reads as container output.
            eprintln!("unhealthy: {host}:{port}{path}: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn a_2xx_is_healthy() {
        let (listener, host, port) = bind();
        let server = serve_once(listener, "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        let status = request(&host, port, "/health").expect("a healthy answer");
        assert_eq!(status, 200);
        server.join().expect("the server thread");
    }

    #[test]
    fn the_request_line_names_the_path_and_the_host() {
        let (listener, host, port) = bind();
        let server = serve_once(listener, "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        let _ = request(&host, port, "/health").expect("a healthy answer");
        let request_line = server.join().expect("the server thread");
        assert_eq!(request_line.trim_end(), "GET /health HTTP/1.0");
    }

    #[test]
    fn a_3xx_is_healthy() {
        // The server redirects `/` to `/search` on purpose. A 3xx is the server
        // routing, not the server failing, and treating it as unhealthy would fail
        // a container that is working exactly as designed.
        let (listener, host, port) = bind();
        let server = serve_once(
            listener,
            "HTTP/1.1 302 Found\r\nLocation: /search\r\nContent-Length: 0\r\n\r\n",
        );
        let status = request(&host, port, "/").expect("a redirect is healthy");
        assert_eq!(status, 302);
        server.join().expect("the server thread");
    }

    #[test]
    fn a_500_is_unhealthy() {
        let (listener, host, port) = bind();
        let server = serve_once(listener, "HTTP/1.1 500 Internal Server Error\r\n\r\n");
        let err = request(&host, port, "/health").expect_err("a 500 is not healthy");
        assert!(
            err.to_string().contains("500"),
            "the error should name the status, got: {err}"
        );
        server.join().expect("the server thread");
    }

    #[test]
    fn a_404_is_unhealthy() {
        let (listener, host, port) = bind();
        let server = serve_once(listener, "HTTP/1.1 404 Not Found\r\n\r\n");
        let err = request(&host, port, "/nope").expect_err("a 404 is not healthy");
        assert!(
            err.to_string().contains("404"),
            "the error should name the status, got: {err}"
        );
        server.join().expect("the server thread");
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
}
