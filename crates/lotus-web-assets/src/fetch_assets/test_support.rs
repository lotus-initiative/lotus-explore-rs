// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! A local HTTP server, because what these fetches decide is what ships.
//!
//! Enough HTTP/1.1 to exercise the status handling with no network and no
//! dependency: one canned response per connection, in order.

// Test scaffolding, not shipped code. It panics and indexes freely by design,
// and its items are named for the test that reads them rather than for a
// contract, so the lint set that protects the fetch paths does not apply here.
// `missing_docs` goes with them: a `///` on every helper would be a comment that
// restates the name, which is the thing to avoid.
#![allow(
    clippy::panic,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    missing_docs,
    missing_debug_implementations
)]

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use reqwest::blocking::Client;

/// Answers one canned response per connection, in order, and records the request
/// line of each.
pub struct MockServer {
    base: String,
    seen: std::sync::mpsc::Receiver<String>,
}

impl MockServer {
    /// Start on a loopback port. `{SELF}` in a response is replaced with this
    /// server's own address, which is how a redirect test points a client back
    /// here.
    pub fn start(responses: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|e| panic!("cannot bind a loopback port: {e}"));
        let port = listener.local_addr().map_or(0, |a| a.port());
        let base = format!("http://127.0.0.1:{port}");
        let (tx, seen) = std::sync::mpsc::channel();
        let self_url = base.clone();
        std::thread::spawn(move || {
            for response in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut buf = [0u8; 2048];
                let read = stream.read(&mut buf).unwrap_or(0);
                let line = String::from_utf8_lossy(buf.get(..read).unwrap_or(&[]))
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                let _ = tx.send(line);
                let _ = stream.write_all(response.replace("{SELF}", &self_url).as_bytes());
                let _ = stream.flush();
            }
        });
        Self { base, seen }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    /// The request line of the next request served, so a test can assert a pinned
    /// version really did short-circuit the network.
    pub fn next_request(&self) -> String {
        self.seen
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("no request was served: {e}"))
    }
}

pub fn http_ok(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

pub fn http_status(code: u16, reason: &str) -> String {
    format!("HTTP/1.1 {code} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
}

/// Accept the connection and close it without sending a response, which is what a
/// load balancer does when it drops a request it has already accepted.
///
/// `reqwest` reports this as a transport error with no status, which is the
/// branch `is_retryable(None)` covers and the one that cannot be reached by queueing
/// an HTTP response: the server has to fail *before* speaking the protocol.
pub fn http_hangup() -> String {
    String::new()
}

pub fn http_redirect_to_self(location: &str) -> String {
    format!(
        "HTTP/1.1 302 Found\r\nLocation: {{SELF}}{location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
}

pub fn client(timeout_ms: u64) -> Client {
    Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .unwrap_or_else(|e| panic!("cannot build a client: {e}"))
}

/// A unique directory under the system temp dir, emptied on the way in.
pub fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("lotus-fetch-{tag}-{}-{unique}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path)
        .unwrap_or_else(|e| panic!("cannot create {}: {e}", path.display()));
    path
}

/// The message of a `Result`'s error, for asserting on it whatever the `Ok` type
/// happens to be.
pub fn err<T>(result: Result<T, Box<dyn std::error::Error>>) -> String {
    match result {
        Ok(_) => panic!("expected an error, got Ok"),
        Err(error) => error.to_string(),
    }
}

/// 40 hex characters, the length every one of these tests needs.
pub fn commit_id(seed: char) -> String {
    std::iter::repeat_n(seed, 40).collect()
}

/// A zip holding `entries`, as `(path in the archive, contents)`.
///
/// Bytes rather than a `zip::ZipWriter`, because `fetch-assets` unzips bytes
/// off the network: building the archive in memory keeps the test off the
/// filesystem and off the network at the same time.
pub fn zip_of(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, contents) in entries {
        writer
            .start_file(*name, options)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        writer
            .write_all(contents.as_bytes())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
    }
    writer
        .finish()
        .unwrap_or_else(|e| panic!("{e}"))
        .into_inner()
}

/// A zip whose single top-level directory is `standalone/`, as Ketcher ships.
pub fn ketcher_style_zip() -> Vec<u8> {
    zip_of(&[
        ("standalone/index.html", "<title>Ketcher v3.18.0</title>"),
        ("standalone/static/js/main.abc.js", "console.log(1)"),
        ("standalone/static/js/closable.def.js", "unused"),
        ("__MACOSX/._standalone", "junk"),
    ])
}
