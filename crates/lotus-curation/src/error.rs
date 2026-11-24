// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Typed errors raised during curation parsing, conversion, and lookup.

use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Which part of curation failed, for deciding whether to retry.
pub enum CurationErrorKind {
    /// The file or a row could not be read.
    InvalidInput,
    /// The request failed.
    Transport,
    /// The answer arrived but could not be read.
    Parse,
}

#[derive(Debug, Error)]
/// A curation failure, at the layer where it can still be classified.
pub enum CurationError {
    /// The input was not usable, with the reason.
    #[error("{0}")]
    InvalidInput(String),
    /// A required column was absent, named so the user can add it.
    #[error("TSV is missing a '{0}' column")]
    MissingTsvColumn(&'static str),
    /// Wikidata or the natural-products API could not be reached.
    #[error("{0}")]
    Http(String),
    /// An answer arrived in a shape this crate does not read.
    #[error("{0}")]
    Parse(String),
}

impl CurationError {
    /// Which part of curation failed, for deciding whether to retry.
    #[must_use]
    pub const fn kind(&self) -> CurationErrorKind {
        match self {
            Self::InvalidInput(_) | Self::MissingTsvColumn(_) => CurationErrorKind::InvalidInput,
            Self::Http(_) => CurationErrorKind::Transport,
            Self::Parse(_) => CurationErrorKind::Parse,
        }
    }

    /// Whether trying again with the same input could plausibly work.
    ///
    /// Only a transport failure is worth retrying, and only because the input was
    /// never in question. Two other failures are final and retrying them just
    /// spends the endpoint's rate limit:
    ///
    /// - a parse failure, because the answer is there and is not in a shape this
    ///   crate reads, so a second request would be refused the same way;
    /// - an input failure, because the same malformed structure is the same
    ///   malformed structure.
    #[must_use]
    pub const fn is_recoverable(&self) -> bool {
        matches!(self, Self::Http(_))
    }
}

#[cfg(test)]
#[path = "error/tests.rs"]
mod tests;
