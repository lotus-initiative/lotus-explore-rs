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
#[path = "healthcheck/tests.rs"]
mod tests;
