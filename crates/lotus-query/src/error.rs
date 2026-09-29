// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The one failure mode of a pure crate: the bytes are not what was expected.

/// A payload that arrived but is not a payload this parser understands.
///
/// Deliberately its own type rather than the caller's: a CSV with the wrong
/// header is a bad answer, not a bad request, and the layer that sends requests
/// is better placed to decide what to tell the user about it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("could not parse the response: {0}")]
pub struct ParseError(String);

impl ParseError {
    /// Builds an error from whatever the underlying reader said.
    pub fn new(message: impl std::fmt::Display) -> Self {
        Self(message.to_string())
    }

    /// The underlying message, for a caller that wants to add context.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.0
    }
}
