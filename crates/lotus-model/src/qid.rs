// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Validated identifier newtypes (`Qid` and `Doi`) so invalid entity or
//! reference identifiers cannot propagate silently past parse boundaries.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::identify::{normalize_doi, strip_qid_digits};
use crate::reference::looks_like_doi;

/// Why a string could not be parsed as a Wikidata [`Qid`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidQidError {
    /// Input was empty or not a Wikidata QID (`Q<digits>` or entity URI).
    #[error("invalid Wikidata QID: {0:?}")]
    InvalidFormat(String),
    /// Numeric portion exceeded `u32::MAX`.
    #[error("Wikidata QID exceeds u32 range: {0:?}")]
    Overflow(String),
}

/// A validated, canonicalized Wikidata entity identifier (`Q<digits>`).
///
/// ```
/// use lotus_model::Qid;
///
/// let qid: Qid = "http://www.wikidata.org/entity/q18216".parse().unwrap();
/// assert_eq!(qid.as_str(), "Q18216");
/// assert_eq!(qid.numeric(), 18_216);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Qid {
    canonical: String,
    numeric: u32,
}

impl Qid {
    /// Construct a [`Qid`] directly from its numeric identifier.
    #[must_use]
    pub fn from_numeric(numeric: u32) -> Self {
        Self {
            canonical: format!("Q{numeric}"),
            numeric,
        }
    }

    /// Canonical `Q<digits>` string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.canonical
    }

    /// Numeric portion of the QID (e.g. `18216` for `Q18216`).
    #[must_use]
    pub const fn numeric(&self) -> u32 {
        self.numeric
    }
}

impl fmt::Display for Qid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical)
    }
}

impl AsRef<str> for Qid {
    fn as_ref(&self) -> &str {
        &self.canonical
    }
}

impl FromStr for Qid {
    type Err = InvalidQidError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let trimmed = raw.trim();
        let entity = trimmed
            .strip_prefix(crate::WIKIDATA_ENTITY_BASE)
            .or_else(|| trimmed.strip_prefix("https://www.wikidata.org/entity/"))
            .unwrap_or(trimmed);
        let normalized = entity
            .strip_prefix('q')
            .map_or_else(|| entity.to_owned(), |rest| format!("Q{rest}"));
        let digits = strip_qid_digits(&normalized)
            .ok_or_else(|| InvalidQidError::InvalidFormat(raw.to_owned()))?;
        let numeric = digits
            .parse::<u32>()
            .map_err(|_| InvalidQidError::Overflow(raw.to_owned()))?;
        Ok(Self::from_numeric(numeric))
    }
}

impl TryFrom<&str> for Qid {
    type Error = InvalidQidError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl Serialize for Qid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.canonical)
    }
}

impl<'de> Deserialize<'de> for Qid {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

/// Why a string could not be parsed as a canonical [`Doi`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid DOI: {0:?}")]
pub struct InvalidDoiError(String);

/// A validated, canonicalized (upper-cased, prefix-stripped) DOI.
///
/// ```
/// use lotus_model::Doi;
///
/// let doi: Doi = "https://doi.org/10.7554/eLife.70780".parse().unwrap();
/// assert_eq!(doi.as_str(), "10.7554/ELIFE.70780");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Doi(String);

impl Doi {
    /// Canonical uppercase, prefix-stripped DOI string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Doi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Doi {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl FromStr for Doi {
    type Err = InvalidDoiError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        if !looks_like_doi(raw) {
            return Err(InvalidDoiError(raw.to_owned()));
        }
        normalize_doi(raw)
            .map(Self)
            .ok_or_else(|| InvalidDoiError(raw.to_owned()))
    }
}

impl TryFrom<&str> for Doi {
    type Error = InvalidDoiError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl Serialize for Doi {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Doi {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
#[path = "qid/tests.rs"]
mod tests;
