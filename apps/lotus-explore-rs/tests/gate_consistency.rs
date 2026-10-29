// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Repository checks the gate can run cheaply.
//!
//! That the local gate, the hooks and CI are one gate is now structural: CI and
//! the hooks only call `./mk`, and the standard `mk` pins and version-checks every
//! tool (`./mk tools`). What remains here is repository state:
//!
//! - [`scrapers`] reads repository files. Nothing here asserts anything.
//! - [`repo_hygiene`] checks repository state the gate can see cheaply.

#![allow(unused_crate_dependencies)] // links the crate's deps without using them

pub mod gate {
    //! The gate-consistency tests, split by concern.
    //!
    //! Nested so `scrapers` can be reached as `crate::gate::scrapers` from the
    //! test modules, which is how a test binary that is not `gate_consistency`
    //! would still use them.

    pub mod repo_hygiene;
    pub mod scrapers;
}
