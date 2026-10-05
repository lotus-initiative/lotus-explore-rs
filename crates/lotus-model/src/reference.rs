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

    #[test]
    fn a_non_ascii_reference_field_is_an_answer_rather_than_a_panic() {
        // Every prefix length is a byte count, so an input whose `prefix.len()`-th
        // byte lands inside a character makes a range slice split that character.
        // These two straddle the boundary: one where the split falls inside the
        // longest prefix, one inside the shortest.
        let straddles_longest = format!("{}\u{3b1}rest", "A".repeat(15));
        let straddles_shortest = format!("{}\u{3b1}rest", "A".repeat(4));
        for text in [&straddles_longest, &straddles_shortest] {
            assert!(!looks_like_doi(text), "{text:?} is not a DOI");
            assert_eq!(strip_doi_prefix(text), text);
            assert!(!looks_like_reference_qid(text));
        }
        // The same shape with a prefix already stripped: a DOI is not required to
        // be ASCII, and the suffix is only checked for emptiness and whitespace,
        // so this has to reach a verdict rather than panic on the way to one.
        assert!(!looks_like_doi("\u{3b1}10.1000/xyz"));
        assert!(looks_like_doi("10.1000/xyz\u{2014}"));
    }
}
