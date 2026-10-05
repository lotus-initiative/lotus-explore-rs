// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The parts of `lotus-healthcheck` that can only be observed by running it.
//!
//! `probe` is unit-tested in the binary against a real socket. What it cannot
//! reach is the step in front: `main` parsing `argv` and turning `probe`'s
//! `ExitCode` into the process's exit status, the only thing Docker reads. A
//! `main` returning the wrong value reports healthy for a server that is not, and
//! that failure is silent — the container stays in the load balancer serving 500s.
//!
//! So this runs the built binary. `CARGO_BIN_EXE_<name>` is the path Cargo built,
//! which means no shell, no `sh -c`, and no dependency on the container image.
// Every binary in this crate shares one dependency list, so an integration test
// for one of them is linked against the dependencies of the other two.
#![allow(
    unused_crate_dependencies,
    reason = "all three binaries in this crate share one dependency list, so this test is linked against the other two's"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "a test that fails to bind a loopback port or spawn the binary cannot report anything else"
)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::Command;

/// A socket that answers `response` to every connection, then stops.
fn serve(response: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let port = listener.local_addr().expect("read the bound port").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            // Read the request line so the client is not refused mid-write, then
            // answer. Not reading is what makes a client report a transport error
            // rather than a status.
            let mut line = String::new();
            if BufReader::new(&stream).read_line(&mut line).is_err() {
                break;
            }
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    port
}

fn healthcheck(port: u16, path: &str) -> i32 {
    Command::new(env!("CARGO_BIN_EXE_lotus-healthcheck"))
        .args(["127.0.0.1", &port.to_string(), path])
        .output()
        .expect("the health check binary was built")
        .status
        .code()
        .expect("the health check exited normally rather than by signal")
}

#[test]
fn a_healthy_server_exits_zero() {
    let port = serve("HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    assert_eq!(healthcheck(port, "/health"), 0, "a 200 is healthy");
}

#[test]
fn a_server_returning_500_exits_nonzero() {
    let port = serve(
        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    );
    assert_ne!(
        healthcheck(port, "/health"),
        0,
        "a 500 is unhealthy, and `main` has to pass that on as the exit status"
    );
}

/// A port nothing is listening on: bound, recorded, released.
fn a_closed_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind a loopback port")
        .local_addr()
        .expect("read the bound port")
        .port()
}

#[test]
fn nothing_listening_exits_nonzero() {
    // A refused connection is the case that takes longest to notice in production,
    // because the server logs nothing at all: there is no request to grep for.
    assert_ne!(
        healthcheck(a_closed_port(), "/health"),
        0,
        "a refused connection is unhealthy"
    );
}

#[test]
fn a_missing_path_on_a_live_server_exits_nonzero() {
    // The path is an argument, so a typo in the Dockerfile's HEALTHCHECK line is a
    // runtime failure rather than a build failure. This is the shape of that: the
    // server answers 200 to everything, and the check has to notice the path is
    // not there rather than reporting whatever the server said.
    let port = serve("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    assert_ne!(healthcheck(port, "/wrong"), 0, "a 404 is unhealthy");
}
