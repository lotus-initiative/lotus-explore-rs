// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Wikidata identifier and DOI normalisation.
//!
//! Single shared implementation used across the workspace.

use super::{WIKIDATA_ENTITY_BASE, WIKIDATA_ENTITY_BASE_HTTPS};

/// Reduce a Wikidata entity URI, typed literal, or bare QID to a bare QID.
///
/// Accepts `http://www.wikidata.org/entity/Q123`, the `https` variant,
/// `"456"^^<…#integer>`, `Q123` and `456`. Anything else — a property QID like
/// `P31`, a blank node, an empty cell — yields an empty string, so a caller can
/// use the result as a key without a second check.
#[must_use]
pub fn normalize_qid(value: &str) -> String {
    strip_qid_digits(value).map_or_else(String::new, |digits| format!("Q{digits}"))
}

/// Extract the ASCII digit slice of a Wikidata QID, URI, or typed integer projection.
///
/// Returns `None` for non-QIDs, blank nodes, or empty inputs without allocating.
#[expect(
    clippy::redundant_pub_crate,
    reason = "private module cannot use `pub` because workspace denies `unreachable_pub`"
)]
pub(crate) fn strip_qid_digits(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    // Both schemes, because Wikidata serves `http` and a mirror or a pasted
    // fixture can carry `https`, and dropping the second would turn a row that
    // used to render into a dropped one.
    let stripped = trimmed
        .strip_prefix(WIKIDATA_ENTITY_BASE)
        .or_else(|| trimmed.strip_prefix(WIKIDATA_ENTITY_BASE_HTTPS))
        .unwrap_or(trimmed);

    // Drop a `"…"^^<…#integer>` wrapper, then any remaining quotes.
    let lexical = stripped
        .split("^^")
        .next()
        .unwrap_or(stripped)
        .trim_matches('"');
    // A bare number is how the `xsd:integer(STRAFTER(STR(?c), "Q"))` projection
    // renders, and how some exports write it.
    let digits = lexical.strip_prefix('Q').unwrap_or(lexical);
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then_some(digits)
}

/// Canonicalise a DOI: strip a `doi.org/` prefix and upper-case the rest.
///
/// Wikidata stores P356 values upper-cased and unprefixed, so an upper-cased
/// value matches an `ASK` that Wikidata will answer. Returns `None` for input
/// with no DOI in it.
#[must_use]
pub fn normalize_doi(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_prefix = crate::strip_doi_prefix(trimmed);
    let canonical = without_prefix.trim();
    if canonical.is_empty() {
        None
    } else {
        Some(canonical.to_ascii_uppercase())
    }
}

/// Fold Unicode subscript digits (`₀`–`₉`) to ASCII (`0`–`9`).
///
/// Wikidata's `P274` property stores chemical formulas with Unicode subscripts
/// (`C₁₇H₁₂O₇`). Folding them in Rust once per compound slot during columnar
/// ingestion avoids running a 10-deep `REPLACE(...)` regex chain on every
/// result row inside the SPARQL `SELECT DISTINCT` projection.
#[must_use]
pub fn normalize_formula_subscripts(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '₀' => '0',
            '₁' => '1',
            '₂' => '2',
            '₃' => '3',
            '₄' => '4',
            '₅' => '5',
            '₆' => '6',
            '₇' => '7',
            '₈' => '8',
            '₉' => '9',
            other => other,
        })
        .collect()
}

/// `Some(s)` only when `s` is non-empty after trimming, so that `Option` and
/// empty-string representations of "absent" do not coexist.
#[must_use]
pub fn non_empty(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t) }
}

/// A bare QID, uppercase or not. At least one digit, so a lone `Q` is a search
/// term rather than an identifier.
#[must_use]
pub fn is_qid(value: &str) -> bool {
    !value.is_empty() && value.trim().len() == value.len() && crate::looks_like_reference_qid(value)
}

/// The name to look up: underscores become spaces, and the genus is
/// capitalised, because taxon names arrive from spreadsheets in both shapes.
#[must_use]
pub fn standardize_taxon_name(input: &str) -> String {
    let replaced = input.replace('_', " ");
    let mut words = replaced.split_whitespace();
    let Some(genus) = words.next() else {
        return replaced;
    };
    let mut chars = genus.chars();
    let mut out = String::with_capacity(replaced.len());
    if let Some(c) = chars.next() {
        out.extend(c.to_uppercase());
        for c in chars {
            out.extend(c.to_lowercase());
        }
    }
    for word in words {
        out.push(' ');
        out.push_str(word);
    }
    out
}

/// Byte-level case-insensitive search. ASCII, because every needle here is an
/// ASCII literal in a URL, DOI, or molfile marker.
#[expect(
    clippy::redundant_pub_crate,
    reason = "private module cannot use `pub` because workspace denies `unreachable_pub`"
)]
pub(crate) fn find_ascii_ci(haystack: &str, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|w| w.iter().zip(needle).all(|(a, b)| a.eq_ignore_ascii_case(b)))
}

#[cfg(test)]
#[path = "identify/tests.rs"]
mod tests;
