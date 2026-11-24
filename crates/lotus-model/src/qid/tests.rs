// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Unit and deterministic property tests for [`Qid`] and [`Doi`].

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "test assertions and property generators"
)]

use super::*;

struct XorShift64(u64);

impl XorShift64 {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

#[test]
fn qid_accepts_canonical_lowercase_and_entity_iris() {
    let q1: Qid = "Q18216".parse().expect("canonical QID");
    let q2: Qid = " q18216 ".parse().expect("lowercase padded QID");
    let q3: Qid = "http://www.wikidata.org/entity/Q18216"
        .parse()
        .expect("http entity URI");
    let q4: Qid = "https://www.wikidata.org/entity/Q18216"
        .parse()
        .expect("https entity URI");
    let q5: Qid = "http://www.wikidata.org/entity/q18216"
        .parse()
        .expect("lowercase http entity URI");
    let q6: Qid = "https://www.wikidata.org/entity/q18216"
        .parse()
        .expect("lowercase https entity URI");

    assert_eq!(q1, q2, "lowercase QID must canonicalize identically");
    assert_eq!(q1, q3, "http URI must canonicalize identically");
    assert_eq!(q1, q4, "https URI must canonicalize identically");
    assert_eq!(q1, q5, "lowercase http URI must canonicalize identically");
    assert_eq!(q1, q6, "lowercase https URI must canonicalize identically");
    assert_eq!(q1.numeric(), 18_216, "numeric accessor mismatch");
    assert_eq!(q1.as_str(), "Q18216", "canonical string mismatch");
}

#[test]
fn qid_rejects_malformed_and_overflowing_inputs() {
    for bad in [
        "",
        "Q",
        "P31",
        "Q12a",
        "https://example.org/Q12",
        "Q999999999999999",
    ] {
        assert!(
            bad.parse::<Qid>().is_err(),
            "Qid::from_str must reject {bad:?}"
        );
    }
}

#[test]
fn qid_round_trips_over_deterministic_generated_u32s() {
    let mut rng = XorShift64(0x9E37_79B9_7F4A_7C15);
    for _ in 0..512 {
        let n = u32::try_from(rng.next() % u64::from(u32::MAX)).unwrap_or(0);
        let q = Qid::from_numeric(n);
        let parsed: Qid = q.as_str().parse().expect("canonical Qid must parse");
        assert_eq!(parsed, q, "Qid round-trip failed for {n}");
        assert_eq!(parsed.numeric(), n, "numeric round-trip failed for {n}");

        let uri = format!("http://www.wikidata.org/entity/Q{n}");
        let from_uri: Qid = uri.parse().expect("entity URI must parse");
        assert_eq!(from_uri, q, "URI Qid equivalence failed for {n}");
    }
}

#[test]
fn doi_strips_prefixes_uppercases_and_rejects_malformed() {
    let d1: Doi = "10.7554/eLife.70780".parse().expect("bare DOI");
    let d2: Doi = "https://doi.org/10.7554/eLife.70780"
        .parse()
        .expect("https DOI");
    let d3: Doi = "DOI:10.7554/ELIFE.70780".parse().expect("doi: prefix");

    assert_eq!(d1, d2, "prefix stripping must produce identical Doi");
    assert_eq!(d1, d3, "case-folding must produce identical Doi");
    assert_eq!(d1.as_str(), "10.7554/ELIFE.70780", "canonical DOI mismatch");

    for bad in ["", "10.12/abc", "10.1000/", "10.1000/a b", "not-a-doi"] {
        assert!(
            bad.parse::<Doi>().is_err(),
            "Doi::from_str must reject {bad:?}"
        );
    }
}

#[test]
fn doi_round_trips_over_deterministic_generated_cases() {
    let mut rng = XorShift64(0xD1B5_4A32_D192_ED03);
    let prefixes = ["", "doi:", "https://doi.org/", "http://dx.doi.org/"];
    for i in 0..512 {
        let registrant = 1000 + (rng.next() % 90_000);
        let suffix_num = rng.next() % 1_000_000;
        let pfx = prefixes.get(i % prefixes.len()).copied().unwrap_or("");
        let raw = format!("{pfx}10.{registrant}/lotus.{suffix_num:x}");
        let doi: Doi = raw.parse().expect("generated DOI must parse");
        let reparsed: Doi = doi.as_str().parse().expect("canonical DOI must reparse");
        assert_eq!(doi, reparsed, "Doi idempotence failed for {raw}");
        assert_eq!(
            doi.as_str(),
            format!("10.{registrant}/LOTUS.{suffix_num:X}"),
            "Doi canonicalization mismatch for {raw}"
        );
    }
}
