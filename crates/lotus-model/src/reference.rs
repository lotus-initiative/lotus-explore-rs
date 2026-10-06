// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Recognising the two things that name a reference: a Wikidata QID and a DOI.
//!
//! A reference is the one entity in a result row a reader cannot name by a short
//! common string: a taxon has a scientific name, a compound a name and an
//! `InChIKey`, but a reference has a title, and matching prose against the
//! reference indices has no useful answer — half of all papers have the same
//! three words as their title.
//!
//! What it does have is two identifiers that each name exactly one item: its
//! Wikidata QID and its DOI. Both are recognised by shape rather than by asking
//! an endpoint, so the field knows which lookup to make without a round trip.

/// Whether this is a Wikidata QID.
///
/// The same test the taxon field has always used, including accepting a lowercase
/// `q` — a reader who types `q18216` means Q18216.
#[must_use]
pub fn looks_like_reference_qid(text: &str) -> bool {
    let trimmed = text.trim();
    let bytes = trimmed.as_bytes();
    // `first()` and `get()` rather than indexing, so the length check above stays
    // the single thing deciding whether this reads past the end.
    bytes.len() > 1
        && matches!(bytes.first(), Some(b'Q' | b'q'))
        && bytes
            .get(1..)
            .is_some_and(|rest| rest.iter().all(u8::is_ascii_digit))
}

/// Whether this is a DOI.
///
/// The registrant prefix is a `NNNN` suffix, the suffix is everything after the
/// first `/`, and both are required: `10.` alone, or a bare `/suffix`, are not
/// DOIs however much they look like one.
///
/// Case is not part of the test, and that is deliberate. DOIs are specified
/// case-insensitively and are commonly written `10.1000/XYZ` in prose, so
/// rejecting an upper-case suffix would refuse a correct DOI.
#[must_use]
pub fn looks_like_doi(text: &str) -> bool {
    let body = strip_doi_prefix(text.trim());
    let Some((prefix, suffix)) = body.split_once('/') else {
        return false;
    };
    // `10.` and four or more digits: the DOI prefix is `NNNN` with at least three.
    let digits = prefix.strip_prefix("10.").unwrap_or("");
    digits.len() >= 3
        && digits.bytes().all(|b| b.is_ascii_digit())
        && !suffix.trim().is_empty()
        && !suffix.contains(char::is_whitespace)
}

/// The DOI proper, without a resolver prefix.
///
/// `https://doi.org/10.1000/xyz`, `doi:10.1000/xyz` and `10.1000/xyz` all name
/// the same DOI, and all three are things a reader pastes from a paper. Wikidata
/// stores the bare form, so the prefixes are stripped before the lookup.
#[must_use]
pub fn strip_doi_prefix(text: &str) -> &str {
    const PREFIXES: [&str; 5] = [
        "https://doi.org/",
        "http://doi.org/",
        "https://dx.doi.org/",
        "http://dx.doi.org/",
        "doi:",
    ];
    let lowered = text.trim();
    // Matched case-insensitively: `HTTPS://DOI.ORG/` is a URL a reader can have.
    for prefix in PREFIXES {
        // `get(..)` rather than a range, because `prefix.len()` is a byte count
        // and the input need not be ASCII. A non-ASCII reader can type a string
        // whose `prefix.len()`-th byte is inside a character, and slicing there
        // panics. `None` is the right answer for that case rather than a reason
        // to keep going: a span ending mid-character cannot equal an ASCII prefix.
        if lowered
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
        {
            // `head` is exactly `prefix.len()` bytes and matched it byte for byte,
            // so the remainder starts on a character boundary.
            return lowered.get(prefix.len()..).unwrap_or(lowered);
        }
    }
    lowered
}

#[cfg(test)]
#[path = "reference/tests.rs"]
mod tests;
