// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Shared stdlib process runner replacing `assert_cmd` and `predicates` in CLI
//! integration tests.

#![allow(
    clippy::expect_used,
    clippy::redundant_pub_crate,
    dead_code,
    reason = "shared integration test helper module compiled independently per test binary"
)]

use std::ffi::OsStr;
use std::io::Write as _;
use std::process::{Command, Output, Stdio};

/// Builder for running the compiled `lotus` binary in integration tests.
pub(crate) struct LotusCmd {
    cmd: Command,
    stdin: Option<Vec<u8>>,
}

/// Captured result of running `lotus`, with fluent status/output assertions.
pub(crate) struct CmdAssert {
    output: Output,
}

/// Returns a command pointing at the `lotus` binary built by Cargo.
pub(crate) fn lotus() -> LotusCmd {
    LotusCmd {
        cmd: Command::new(env!("CARGO_BIN_EXE_lotus")),
        stdin: None,
    }
}

impl LotusCmd {
    pub(crate) fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.cmd.arg(arg);
        self
    }

    pub(crate) fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.cmd.args(args);
        self
    }

    pub(crate) fn env<K, V>(&mut self, key: K, val: V) -> &mut Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.cmd.env(key, val);
        self
    }

    pub(crate) fn env_remove<K: AsRef<OsStr>>(&mut self, key: K) -> &mut Self {
        self.cmd.env_remove(key);
        self
    }

    pub(crate) fn write_stdin(&mut self, input: impl Into<Vec<u8>>) -> &mut Self {
        self.stdin = Some(input.into());
        self
    }

    pub(crate) fn output(&mut self) -> std::io::Result<Output> {
        let Some(bytes) = self.stdin.take() else {
            return self.cmd.output();
        };
        let mut child = self
            .cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let Some(mut pipe) = child.stdin.take() {
            pipe.write_all(&bytes)?;
        }
        child.wait_with_output()
    }

    pub(crate) fn assert(&mut self) -> CmdAssert {
        let output = self.output().expect("the lotus binary runs");
        CmdAssert { output }
    }
}

impl CmdAssert {
    pub(crate) fn success(self) -> Self {
        assert!(
            self.output.status.success(),
            "expected success, got {}\nstdout:\n{}\nstderr:\n{}",
            self.output.status,
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr),
        );
        self
    }

    pub(crate) fn failure(self) -> Self {
        assert!(
            !self.output.status.success(),
            "expected failure, got success\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr),
        );
        self
    }

    pub(crate) fn code(self, expected: i32) -> Self {
        assert_eq!(
            self.output.status.code(),
            Some(expected),
            "expected exit code {expected}, got {:?}\nstdout:\n{}\nstderr:\n{}",
            self.output.status.code(),
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr),
        );
        self
    }

    pub(crate) fn stdout_contains(self, needle: &str) -> Self {
        let stdout = String::from_utf8_lossy(&self.output.stdout);
        assert!(
            stdout.contains(needle),
            "expected stdout to contain {needle:?}\nstdout:\n{stdout}"
        );
        self
    }

    pub(crate) fn stdout_not_contains(self, needle: &str) -> Self {
        let stdout = String::from_utf8_lossy(&self.output.stdout);
        assert!(
            !stdout.contains(needle),
            "expected stdout not to contain {needle:?}\nstdout:\n{stdout}"
        );
        self
    }

    pub(crate) fn stderr_contains(self, needle: &str) -> Self {
        let stderr = String::from_utf8_lossy(&self.output.stderr);
        assert!(
            stderr.contains(needle),
            "expected stderr to contain {needle:?}\nstderr:\n{stderr}"
        );
        self
    }

    pub(crate) const fn get_output(&self) -> &Output {
        &self.output
    }
}
