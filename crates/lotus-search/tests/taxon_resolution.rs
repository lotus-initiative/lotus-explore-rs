// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

//! What `resolve_taxon` tells the reader about what it did.
//!
//! Four branches with no test between them: the miss, the standardised-input
//! notice, the ambiguity notice, and the cap on how many candidates that notice
//! lists. The first three are what the user reads after a search returns
//! something they did not ask for -- a wrong taxon, or a silently substituted
//! one -- and the cap is the difference between a notice and a wall of text.
//
// Everything runs against `Scripted`. No socket.

// The panic lints keep library code free of panics on external input. A test
// failing on a bad script is reporting, not panicking.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus_search::TaxonNote;
use lotus_search::testing::Scripted;
use lotus_search::{FetchError, resolve_taxon};

/// What the taxon lookup answers: `(uri, name, matched_by)` per row.
fn lookup_csv(rows: &[(&str, &str, &str)]) -> String {
    use std::fmt::Write as _;
    let mut csv = String::from("taxon,taxon_name,matched_by\n");
    for (uri, name, source) in rows {
        let _ = writeln!(csv, "{uri},{name},{source}");
    }
    csv
}

#[tokio::test]
async fn a_taxon_that_matched_nothing_is_an_error_naming_what_was_looked_for() {
    // The rule the FAQ states: a name that matches nothing is an error, because
    // there is nothing to guess at. `Ok` with no QID would leave the caller
    // searching all of LOTUS while the reader believes they narrowed it.
    let http = Scripted::new(vec![(200, &lookup_csv(&[]))]);
    let error = resolve_taxon(&http, "Gentiana lutea")
        .await
        .expect_err("a name that matched nothing is an error");
    assert!(
        matches!(&error, FetchError::Parse(message) if message.contains("Gentiana lutea")),
        "the error must name what was searched for, got {error:?}"
    );
}

#[tokio::test]
async fn a_lookup_that_answered_with_no_usable_row_is_also_a_miss() {
    // The endpoint answered 200 with a header and nothing usable. That is the
    // same answer to the reader's question -- nothing matched -- so it takes the
    // same path rather than surfacing as something stranger.
    let http = Scripted::new(vec![(200, "taxon,taxon_name,matched_by\n")]);
    resolve_taxon(&http, "Rosa")
        .await
        .expect_err("no usable row is a miss");
}

#[tokio::test]
async fn an_underscored_lower_case_genus_is_standardised_and_says_so() {
    // Both halves arrive from spreadsheets. The query goes out with the
    // standardised text, and the notice reports the rewrite -- silently
    // searching different text than the reader typed is the thing the notice
    // exists to prevent.
    let http = Scripted::new(vec![(
        200,
        &lookup_csv(&[(
            "http://www.wikidata.org/entity/Q16521",
            "Gentiana lutea",
            "scientific",
        )]),
    )]);
    let resolution = resolve_taxon(&http, "gentiana_lutea")
        .await
        .expect("the lookup answers");

    assert_eq!(resolution.qid.as_deref(), Some("Q16521"));
    let standardized = resolution
        .notes
        .iter()
        .find_map(|note| match note {
            TaxonNote::Standardized {
                original,
                looked_up,
            } => Some((original.as_str(), looked_up.as_str())),
            TaxonNote::Ambiguous { .. } => None,
        })
        .expect("the rewrite must be reported");
    assert_eq!(
        standardized,
        ("gentiana_lutea", "Gentiana lutea"),
        "the notice must name both what was typed and what was looked up"
    );
}

#[tokio::test]
async fn a_name_needing_no_rewrite_carries_no_standardised_notice() {
    // The notice is a warning, so emitting one for an untouched name trains the
    // reader to ignore it.
    let http = Scripted::new(vec![(
        200,
        &lookup_csv(&[(
            "http://www.wikidata.org/entity/Q16521",
            "Rosa",
            "scientific",
        )]),
    )]);
    let resolution = resolve_taxon(&http, "Rosa")
        .await
        .expect("the lookup answers");
    assert!(
        !resolution
            .notes
            .iter()
            .any(|note| matches!(note, TaxonNote::Standardized { .. })),
        "nothing was rewritten, so there is nothing to report: {:?}",
        resolution.notes
    );
}

#[tokio::test]
async fn several_matches_produce_an_ambiguity_notice_naming_the_alternatives() {
    // More than one match and the first was used. The reader cannot tell from a
    // row of results which taxon was actually searched, so the alternatives are
    // named.
    let rows = [
        (
            "http://www.wikidata.org/entity/Q16521",
            "Gentiana lutea",
            "scientific",
        ),
        (
            "http://www.wikidata.org/entity/Q131960",
            "Gentiana",
            "scientific",
        ),
    ];
    let http = Scripted::new(vec![(200, &lookup_csv(&rows))]);
    let resolution = resolve_taxon(&http, "Gentiana lutea")
        .await
        .expect("the lookup answers");

    let ambiguous = resolution
        .notes
        .iter()
        .find_map(|note| match note {
            TaxonNote::Ambiguous { candidates } => Some(candidates),
            TaxonNote::Standardized { .. } => None,
        })
        .expect("more than one match must be reported");
    assert_eq!(
        ambiguous,
        &vec![
            "Gentiana lutea (Q16521)".to_string(),
            "Gentiana (Q131960)".to_string()
        ],
        "each alternative must be named with its QID, so the reader can look it up"
    );
    assert_eq!(
        resolution.looked_up, "Gentiana lutea",
        "the text looked up is reported, so the reader can see what was searched"
    );
}

#[tokio::test]
async fn the_ambiguity_notice_lists_four_and_not_every_match() {
    // A common name can match dozens of taxa. The notice has to stay a notice:
    // the cap is what keeps it on one line of the results panel.
    let many: Vec<(String, String, String)> = (1..=12)
        .map(|n| {
            (
                format!("http://www.wikidata.org/entity/Q{:04}", 1000 + n),
                format!("Rosa {n}"),
                "common".to_string(),
            )
        })
        .collect();
    let refs: Vec<(&str, &str, &str)> = many
        .iter()
        .map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()))
        .collect();
    let http = Scripted::new(vec![(200, &lookup_csv(&refs))]);
    let resolution = resolve_taxon(&http, "Rosa")
        .await
        .expect("the lookup answers");

    let ambiguous = resolution
        .notes
        .iter()
        .find_map(|note| match note {
            TaxonNote::Ambiguous { candidates } => Some(candidates),
            TaxonNote::Standardized { .. } => None,
        })
        .expect("more than one match must be reported");
    assert_eq!(
        ambiguous.len(),
        4,
        "a dozen matches must not become a dozen lines of notice: {ambiguous:?}"
    );
    // And the ones listed are the first ones, so the notice leads with what was
    // most likely meant.
    assert_eq!(
        ambiguous[0], "Rosa 1 (Q1001)",
        "the notice leads with the first match"
    );
}

#[tokio::test]
async fn a_wildcard_taxon_is_a_query_choice_and_not_a_lookup() {
    // `*` means every taxon with a recorded occurrence, so it is answered
    // locally rather than asking Wikidata to match a literal asterisk.
    let http = Scripted::new(vec![(200, "should never be read")]);
    let resolution = resolve_taxon(&http, "*")
        .await
        .expect("a wildcard needs no lookup");
    assert_eq!(http.call_count(), 0, "a wildcard must not be looked up");
    assert_eq!(
        resolution.qid, None,
        "and it resolves to no QID, because it names no taxon"
    );
    assert!(
        resolution.notes.is_empty(),
        "nothing was standardised and nothing was ambiguous: {:?}",
        resolution.notes
    );
}
