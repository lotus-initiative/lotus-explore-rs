// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The wire format the REST API speaks.
//!
//! These are the request and response shapes of `/v1/search`, kept apart from
//! the client that sends them: the client is a side effect and belongs to the
//! host, the shapes are the contract and belong to the domain.

pub mod dto;
pub mod error;
