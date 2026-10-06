// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `identify`, in their own file.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use super::*;

#[test]
fn qids_arrive_in_four_shapes_and_leave_as_one() {
    for input in [
        "http://www.wikidata.org/entity/Q12345",
        "https://www.wikidata.org/entity/Q12345",
        "\"456\"^^<http://www.w3.org/2001/XMLSchema#integer>",
        "Q789",
        "456",
        "  Q1  ",
    ] {
        let out = normalize_qid(input);
        assert!(
            out.starts_with('Q') && out[1..].bytes().all(|b| b.is_ascii_digit()),
            "{input:?} produced {out:?}"
        );
    }
    assert_eq!(
        normalize_qid("http://www.wikidata.org/entity/Q12345"),
        "Q12345"
    );
    assert_eq!(
        normalize_qid("\"456\"^^<http://www.w3.org/2001/XMLSchema#integer>"),
        "Q456"
    );
    assert_eq!(normalize_qid("456"), "Q456");
}

#[test]
fn a_non_entity_yields_nothing() {
    // A property QID, a lexeme, a blank node and an empty cell are all
    // "no entity here" — the caller keys on the result directly.
    for input in [
        "P31",
        "L123",
        "_:b0",
        "",
        "   ",
        "Q",
        "http://example.org/Q1",
    ] {
        assert_eq!(normalize_qid(input), "", "input {input:?}");
    }
}

#[test]
fn dois_are_prefixed_stripped_and_upper_cased() {
    assert_eq!(normalize_doi("10.1/a").as_deref(), Some("10.1/A"));
    assert_eq!(
        normalize_doi("https://doi.org/10.1/a").as_deref(),
        Some("10.1/A")
    );
    assert_eq!(
        normalize_doi("HTTPS://DOI.ORG/10.1000/ABC").as_deref(),
        Some("10.1000/ABC")
    );
    assert_eq!(normalize_doi("  10.1/B  ").as_deref(), Some("10.1/B"));
}

#[test]
fn a_doi_with_nothing_in_it_is_absent() {
    for input in ["", "   ", "https://doi.org/", "doi.org/"] {
        assert_eq!(normalize_doi(input), None, "input {input:?}");
    }
}

#[test]
fn emptiness_is_measured_after_trimming() {
    assert_eq!(non_empty("  x "), Some("x"));
    assert_eq!(non_empty(""), None);
    assert_eq!(non_empty("  \t "), None);
}

#[test]
fn a_case_insensitive_search_finds_any_single_byte() {
    // The window test is per byte, and a needle that is not ASCII is
    // compared byte-wise too. What matters is that one wrong byte in the
    // middle stops the match rather than being skipped.
    assert_eq!(find_ascii_ci("Hello World", b"world"), Some(6));
    assert_eq!(find_ascii_ci("Hello World", b"WORLD"), Some(6));
    assert_eq!(find_ascii_ci("Hello World", b"o w"), Some(4));
    assert_eq!(find_ascii_ci("Hello World", b"o_W"), None);
    assert_eq!(
        find_ascii_ci("Hello World", b"World!"),
        None,
        "a partial match is not a match"
    );
    assert_eq!(
        find_ascii_ci("hi", b""),
        None,
        "an empty needle is not found anywhere"
    );
    assert_eq!(find_ascii_ci("", b"x"), None, "nothing to find in nothing");
    assert_eq!(
        find_ascii_ci("a", b"ab"),
        None,
        "a needle longer than the haystack"
    );
}

#[test]
fn the_search_does_not_match_across_a_multibyte_character() {
    // Byte windows can straddle a UTF-8 boundary, where a match would be a
    // coincidence of continuation bytes rather than a real substring.
    // n(0) a(1) U+00EF(2,3) v(4) e(5): the needle starts after the two-byte character.
    assert_eq!(find_ascii_ci("na\u{ef}ve x\u{e9}y", b"ve"), Some(4));
    // A needle that is not valid ASCII has no meaningful case folding here.
    assert_eq!(find_ascii_ci("na\u{ef}ve", &[0xef, 0x76]), None);
}
