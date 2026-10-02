// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Recognising the two things that name a reference: a Wikidata QID and a DOI.
//!
//! A reference is the one entity in a result row that a reader cannot name by a
//! short common string. A taxon has a scientific name and a common one. A
//! compound has a name, a formula and an `InChIKey`. A reference has a title,
//! which is prose, and matching prose against the reference indices is a question
//! with no useful answer — every paper has a title and half of them are the same
//! three words.
//!
//! What a reference *does* have is two identifiers that each name exactly one
//! item: its Wikidata QID, and its DOI. Those are what this module recognises,
//! and they are recognised by shape rather than by asking an endpoint first, so
//! the field knows which lookup to make without a round trip.

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
        if lowered.len() >= prefix.len() && lowered[..prefix.len()].eq_ignore_ascii_case(prefix) {
            return &lowered[prefix.len()..];
        }
    }
    lowered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_qid_is_recognised_in_either_case() {
        assert!(looks_like_reference_qid("Q18216"));
        assert!(looks_like_reference_qid("q18216"));
        assert!(looks_like_reference_qid("  Q18216  "));
        assert!(looks_like_reference_qid("Q1"));
    }

    #[test]
    fn something_that_is_not_a_qid_is_not_one() {
        // `Q` alone is a prefix a reader has not finished typing, and the rest
        // are not identifiers.
        for text in [
            "",
            "   ",
            "Q",
            "q",
            "Q18a16",
            "CCQ",
            "quinine",
            "10.1000/xyz",
        ] {
            assert!(!looks_like_reference_qid(text), "{text} is not a QID");
        }
    }

    #[test]
    fn a_doi_is_recognised() {
        assert!(looks_like_doi("10.1000/xyz"));
        assert!(looks_like_doi("10.1002/andp.18280880206"));
        assert!(looks_like_doi("10.1000/xyz789"));
    }

    #[test]
    fn a_doi_arrives_with_any_of_the_prefixes_a_reader_pastes() {
        // All four of these turn up in a paper, an email and a reference manager.
        for text in [
            "10.1000/xyz",
            "doi:10.1000/xyz",
            "https://doi.org/10.1000/xyz",
            "http://dx.doi.org/10.1000/xyz",
        ] {
            assert!(looks_like_doi(text), "{text} should be a DOI");
            assert_eq!(strip_doi_prefix(text), "10.1000/xyz");
        }
    }

    #[test]
    fn a_doi_suffix_may_be_upper_case() {
        // DOIs are case-insensitive and are routinely written that way in prose.
        assert!(looks_like_doi("10.1000/XYZ"));
        assert_eq!(strip_doi_prefix("10.1000/XYZ"), "10.1000/XYZ");
    }

    #[test]
    fn something_that_is_not_a_doi_is_not_one() {
        for text in [
            "",
            "   ",
            "10.",
            "10.10/x",     // two-digit registrant prefix
            "11.1000/xyz", // not the DOI prefix
            "1000/xyz",    // no `10.`
            "10.1000/",    // nothing after the slash
            "10.1000/  ",  // blank suffix
            "10.1000/a b", // whitespace inside
            "abc/def",
            "Gentiana lutea",
        ] {
            assert!(!looks_like_doi(text), "{text} should not be a DOI");
        }
    }

    #[test]
    fn a_qid_is_never_mistaken_for_a_doi_and_the_other_way_round() {
        // The two routes have to be disjoint, or a QID would cost a pointless
        // round trip asking Wikidata to look up a DOI it already holds.
        assert!(!looks_like_doi("Q18216"));
        assert!(!looks_like_reference_qid("10.1000/xyz"));
    }
}
