// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Test target for `build.rs`.
//!
//! `cargo test` does not compile `build.rs` as a test target, so its `#[test]`s
//! never run (a test there reports "0 passed" under `--all-targets`).
//!
//! This exists rather than naming `build.rs` as the target's own path, because
//! that would make one file the root of two targets and cargo warns:
//!
//!   warning: file `build.rs` found to be present in multiple build targets:
//!     * `integration-test` target `buildrs`
//!     * `build-script` target `build-script-build`
//!
//! Pulling it in as a module keeps `build.rs` the root of exactly one target,
//! and the tests stay in the file whose logic they cover.

// Crate-level allows for the build script pulled in below, which cannot carry
// inner attributes of its own once it is a module. `unused_crate_dependencies`
// is ignored inside a module, hence here rather than on the `mod`.
#![allow(
    unused_crate_dependencies,
    reason = "the build script is pulled in as a module of a test target, so the crate-attribute has to be here rather than on the `mod`"
)]
// `fn main` is dead code here: this target runs the tests, not the build script.
#![allow(
    dead_code,
    reason = "`fn main` belongs to the build-script target; this target runs the tests in the same file"
)]

#[path = "build.rs"]
mod build_script;
