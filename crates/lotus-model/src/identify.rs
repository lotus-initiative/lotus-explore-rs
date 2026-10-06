// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! Wikidata identifier and DOI normalisation.
//!
//! The same three jobs were implemented six times across the workspace. These
//! are the only copies.

use super::WIKIDATA_ENTITY_BASE;

const WIKIDATA_ENTITY_BASE_HTTPS: &str = "https://www.wikidata.org/entity/";

/// Reduce a Wikidata entity URI, typed literal, or bare QID to a bare QID.
///
/// Accepts `http://www.wikidata.org/entity/Q123`, the `https` variant,
/// `"456"^^<…#integer>`, `Q123` and `456`. Anything else — a property QID like
/// `P31`, a blank node, an empty cell — yields an empty string, so a caller can
/// use the result as a key without a second check.
#[must_use]
pub fn normalize_qid(value: &str) -> String {
    let trimmed = value.trim();
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
    let Some(candidate) = lexical.strip_prefix('Q') else {
        return numeric_to_qid(lexical);
    };

    if !candidate.is_empty() && candidate.bytes().all(|b| b.is_ascii_digit()) {
        lexical.to_string()
    } else {
        String::new()
    }
}

fn numeric_to_qid(lexical: &str) -> String {
    if !lexical.is_empty() && lexical.bytes().all(|b| b.is_ascii_digit()) {
        format!("Q{lexical}")
    } else {
        String::new()
    }
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
    let without_prefix = find_ascii_ci(trimmed, b"doi.org/")
        .map_or(trimmed, |idx| &trimmed[idx + "doi.org/".len()..]);
    let canonical = without_prefix.trim();
    if canonical.is_empty() {
        None
    } else {
        Some(canonical.to_ascii_uppercase())
    }
}

/// `Some(s)` only when `s` is non-empty after trimming, so that `Option` and
/// empty-string representations of "absent" do not coexist.
#[must_use]
pub fn non_empty(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t) }
}

/// Byte-level case-insensitive search. ASCII, because every needle here is an
/// ASCII literal in a URL or a DOI.
fn find_ascii_ci(haystack: &str, needle: &[u8]) -> Option<usize> {
    let hay = haystack.as_bytes();
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len())
        .position(|w| w.iter().zip(needle).all(|(a, b)| a.eq_ignore_ascii_case(b)))
}

#[cfg(test)]
#[path = "identify/tests.rs"]
mod tests;
