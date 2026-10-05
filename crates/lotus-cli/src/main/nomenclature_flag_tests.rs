// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The flags-to-criteria mapping, checked without a network or a binary.
//!
//! `criteria_from_args` is the whole of the CLI's contribution to a search, and
//! otherwise reachable only by running `lotus search` and reading the SPARQL it
//! prints. That cannot cover these flags: `--explain` deliberately skips taxon
//! resolution, so a taxon-filtered query is never built and the nomenclatural
//! path never emitted.

#![allow(clippy::expect_used, clippy::panic)]

use super::*;

/// Parse a command line the way the binary does, and return its `search` args.
///
/// Through the real parser rather than by constructing `SearchArgs` by hand, so
/// that a flag whose clap declaration and whose field name drift apart is
/// caught here rather than by a user.
fn args_from(argv: &[&str]) -> SearchArgs {
    use clap::Parser;
    let mut full = vec!["lotus", "search"];
    full.extend_from_slice(argv);
    match Cli::try_parse_from(full)
        .expect("the arguments parse")
        .command
    {
        Command::Search(parsed) => parsed,
        other => panic!("expected a search command, got {other:?}"),
    }
}

const NOW: u16 = 2026;

/// Each `--no-*` flag turns its own relationship off, and nothing else.
///
/// The per-relationship shape is the point. Asserting only that "some flag
/// turns something off" is satisfied by a mapping that sends all four flags at
/// whichever relationship happens to be checked first, so each is checked
/// against the other three staying on.
#[test]
fn each_no_flag_turns_off_only_its_own_relationship() {
    use lotus_model::taxon_nomenclature as n;

    for (flag, id) in [
        ("--no-accepted-synonyms", n::ACCEPTED_SYNONYM.id),
        ("--no-basionyms", n::BASIONYM.id),
        ("--no-protonyms", n::PROTONYM.id),
        ("--no-replacements", n::REPLACEMENT.id),
    ] {
        let args = args_from(&[flag]);
        let names = criteria_from_args(&args, NOW).taxon_names;

        for relation in n::ALL {
            let expected = relation.id != id;
            assert_eq!(
                names.for_relation(&relation),
                expected,
                "{flag} left {} wrong",
                relation.id
            );
        }
    }
}

/// No flag means all four relationships are followed.
///
/// The `!` in the mapping is the hazard this pins: dropping it makes
/// `--no-basionyms` a way of *enabling* the basionym search, which is the exact
/// inverse of what the flag says, and is invisible in a printed query only if
/// the reader does not know which way round to look.
#[test]
fn passing_no_nomenclatural_flag_follows_all_four() {
    use lotus_model::taxon_nomenclature as n;

    let names = criteria_from_args(&args_from(&["--taxon", "Q16521"]), NOW).taxon_names;
    assert_eq!(names, lotus_model::TaxonNomenclature::ALL_ON);
    for relation in n::ALL {
        assert!(
            names.for_relation(&relation),
            "{} should be on",
            relation.id
        );
    }
}

/// A flag's absence and its presence are different inputs, and they have to
/// give different answers.
#[test]
fn a_negated_flag_and_an_absent_one_disagree() {
    let absent = criteria_from_args(&args_from(&[]), NOW).taxon_names;
    let present = criteria_from_args(&args_from(&["--no-basionyms"]), NOW).taxon_names;
    assert_ne!(
        absent, present,
        "--no-basionyms must actually change something"
    );
}

/// Two flags at once turn off two relationships and leave the other two alone,
/// which is the state the URL codec's absent-versus-false handling is about.
#[test]
fn two_flags_turn_off_exactly_two() {
    use lotus_model::taxon_nomenclature as n;

    let names =
        criteria_from_args(&args_from(&["--no-basionyms", "--no-replacements"]), NOW).taxon_names;

    assert!(!names.for_relation(&n::BASIONYM));
    assert!(!names.for_relation(&n::REPLACEMENT));
    assert!(names.for_relation(&n::ACCEPTED_SYNONYM));
    assert!(names.for_relation(&n::PROTONYM));
    assert!(names.any(), "two are still on");
}

/// All four off is a search for the taxon name as typed, and the query builder
/// has to be able to produce it.
#[test]
fn all_four_off_is_representable_and_reaches_the_query() {
    let args = args_from(&[
        "--no-accepted-synonyms",
        "--no-basionyms",
        "--no-protonyms",
        "--no-replacements",
    ]);
    let names = criteria_from_args(&args, NOW).taxon_names;
    assert!(!names.any());

    let query = lotus_search::build_base_query(
        &lotus_model::SearchCriteria {
            taxon_names: names,
            ..lotus_model::SearchCriteria::up_to_year(NOW)
        },
        Some("Q16521"),
    );
    assert!(
        query.contains("?t (wdt:P171*) wd:Q16521 ."),
        "the name as typed, with no closure:\n{query}"
    );
}
