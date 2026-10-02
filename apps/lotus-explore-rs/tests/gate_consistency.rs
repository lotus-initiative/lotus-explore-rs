// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The local gate, the git hooks and the CI gate must be the same gate.
//!
//! Four files, one per concern, because this grew to a point where the module
//! documentation was describing a file nobody could hold in their head:
//!
//! - [`scrapers`] reads the four config files and answers "which names appear
//!   under this key". Nothing here asserts anything.
//! - [`job_map`] holds the correspondence between CI jobs, local tasks and hooks,
//!   and the tests that hold it honest.
//! - [`ci_workflow`] checks that the workflow installs what it runs, and that the
//!   tools it pins agree with the local gate's.
//! - [`repo_hygiene`] checks repository state the gate can see cheaply.
//!
//! Adding a check means adding it to the file for its concern, which is the point:
//! the previous single file made "where does this go" a question you had to
//! answer by reading all of it.

#![allow(unused_crate_dependencies)] // links the crate's deps without using them

pub mod gate {
    //! The gate-consistency tests, split by concern.
    //!
    //! Nested so `scrapers` can be reached as `crate::gate::scrapers` from the
    //! test modules, which is how a test binary that is not `gate_consistency`
    //! would still use them.

    pub mod ci_workflow;
    pub mod job_map;
    pub mod repo_hygiene;
    pub mod scrapers;
}
