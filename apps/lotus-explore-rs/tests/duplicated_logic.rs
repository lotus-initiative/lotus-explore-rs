// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Characterization tests for the two search paths that were written twice.
//!
//! The web pipeline and the `/v1` API each build a query and each resolve a
//! taxon. They are not equivalent. These tests pin both behaviours *as they are
//! today* so that consolidating them is a deliberate decision rather than an
//! accident, and so a reviewer can see exactly what consolidating costs.

// See `crates/lotus/tests/result_parsing.rs` for why the panic lints are
// relaxed in tests.
#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use lotus::models::{SearchCriteria, SmilesSearchType};
use lotus::queries::{self, StructureKind};

const MOLFILE: &str = "\n\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\n    0.0000    0.0000    0.0000 C   0  0  0  0  0  0\nM  END\n";

/// Structure normalisation, as implemented in both
/// `features/explore/service/build_query.rs` and `server/query_logic.rs`. The
/// two copies are byte-identical today, so one copy stands for both.
fn normalize_structure(value: &str) -> String {
    let normalized = if value.contains('\r') {
        value.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        value.to_string()
    };
    match queries::classify_structure(&normalized) {
        StructureKind::MolfileV2000 | StructureKind::MolfileV3000 => normalized,
        _ => normalized.trim().to_string(),
    }
}

#[test]
fn structure_normalisation_unifies_line_endings_and_trims_smiles() {
    assert_eq!(normalize_structure("  CCO  "), "CCO");
    assert_eq!(normalize_structure("a\r\nb\rc"), "a\nb\nc");
    // A molfile keeps its interior structure, including the leading blank lines.
    assert!(normalize_structure(MOLFILE).starts_with("\n\n\n  1  0"));
}

/// The web pipeline, copied from `features/explore/service/build_query.rs`.
fn web_build_query(crit: &SearchCriteria, taxon_qid: Option<&str>) -> String {
    let smiles = normalize_structure(&crit.smiles);
    if smiles.is_empty() {
        return match taxon_qid {
            Some(qid) if qid != "*" => queries::query_compounds_by_taxon(qid),
            _ => queries::query_all_compounds(),
        };
    }
    let effective_type = if (smiles.contains('\n') || smiles.contains('\r'))
        && crit.smiles_search_type == SmilesSearchType::Similarity
    {
        SmilesSearchType::Substructure
    } else {
        crit.smiles_search_type
    };
    let taxon_for_sachem = match taxon_qid {
        Some("*") => Some("Q2382443"),
        Some(qid) => Some(qid),
        None => None,
    };
    queries::query_sachem(
        &smiles,
        effective_type,
        crit.smiles_threshold,
        taxon_for_sachem,
    )
}

/// The API, copied from `server/query_logic.rs`.
fn api_build_query(crit: &SearchCriteria, taxon_qid: Option<&str>) -> String {
    let smiles = normalize_structure(&crit.smiles);
    if smiles.is_empty() {
        return match taxon_qid {
            Some("*") | None => queries::query_all_compounds(),
            Some(qid) => queries::query_compounds_by_taxon(qid),
        };
    }
    let taxon_for_sachem = match taxon_qid {
        Some("*") => Some("Q2382443"),
        Some(qid) => Some(qid),
        None => None,
    };
    queries::query_sachem(
        &smiles,
        crit.smiles_search_type,
        crit.smiles_threshold,
        taxon_for_sachem,
    )
}

#[test]
fn both_paths_produce_the_same_query_when_there_is_no_structure() {
    for taxon in [None, Some("*"), Some("Q16521")] {
        let crit = SearchCriteria {
            taxon: "Gentiana lutea".into(),
            ..SearchCriteria::default()
        };
        assert_eq!(
            web_build_query(&crit, taxon),
            api_build_query(&crit, taxon),
            "taxon {taxon:?}"
        );
    }
}

#[test]
fn the_api_does_not_downgrade_a_molfile_similarity_search() {
    // The web pipeline falls back to substructure when a *multiline* structure
    // is asked for with similarity, because Sachem's similarity service cannot
    // take a molfile. The API has no such guard: it sends the similarity query
    // with a triple-quoted molfile literal and the endpoint rejects it.
    //
    // This is a real behavioural difference, and it is the one to decide about
    // when the two paths are merged. Adopting the web behaviour is the fix;
    // it is a change to the API's responses, not to the web app's.
    let crit = SearchCriteria {
        smiles: "\n\n\n  1  0  0  0  0  0  0  0  0  0999 V2000\nM  END\n".into(),
        smiles_search_type: SmilesSearchType::Similarity,
        ..SearchCriteria::default()
    };

    let web = web_build_query(&crit, Some("Q16521"));
    let api = api_build_query(&crit, Some("Q16521"));

    assert!(
        web.contains("sachem:scoredSubstructureSearch"),
        "web downgrades"
    );
    assert!(web.contains("sachem:searchMode sachem:substructureSearch"));

    assert!(api.contains("sachem:similarCompoundSearch"), "API does not");
    assert!(api.contains("sachem:cutoff"));
    assert!(
        api.contains("'''"),
        "and sends a triple-quoted molfile literal"
    );
}

/// The two "is this already a QID?" checks, copied from
/// `resolve_taxon/mod.rs` and `server/query_logic.rs`.
fn web_is_bare_qid(taxon: &str) -> bool {
    !taxon.is_empty() && taxon != "*" && {
        let bytes = taxon.as_bytes();
        bytes.len() > 1
            && matches!(bytes[0], b'Q' | b'q')
            && bytes[1..].iter().all(u8::is_ascii_digit)
    }
}

fn api_is_bare_qid(value: &str) -> bool {
    let v = value.trim();
    let mut chars = v.chars();
    matches!(chars.next(), Some('Q' | 'q')) && !v.is_empty() && chars.all(|c| c.is_ascii_digit())
}

#[test]
fn the_two_bare_qid_checks_agree_on_realistic_input() {
    for input in ["Q12345", "q12345", "Gentiana lutea", "", "*", "Q12a3"] {
        assert_eq!(
            web_is_bare_qid(input),
            api_is_bare_qid(input),
            "input {input:?}"
        );
    }
}

#[test]
fn a_lone_q_is_treated_as_a_qid_only_by_the_api() {
    // The API check never requires a second character, so a bare "Q" is passed
    // straight through to the query as if it were a Wikidata id. The web check
    // requires at least one digit, so it does a name lookup instead and reports
    // "taxon not found". The web behaviour is the intended one.
    assert!(api_is_bare_qid("Q"), "API: no digit requirement");
    assert!(!web_is_bare_qid("Q"), "web: a digit is required");
}

#[test]
fn both_checks_accept_a_leading_or_trailing_space_only_differently() {
    // The web check looks at the raw string; the API trims first. A user who
    // pastes " Q12345 " gets a remote lookup from the API and a direct hit from
    // the web app.
    assert!(!web_is_bare_qid(" Q12345 "));
    assert!(api_is_bare_qid(" Q12345 "));
}

/// The taxon sanitiser, copied from `search_utils::sanitize_taxon_input`.
/// Underscores become spaces, the first word is title-cased, the rest is left
/// as typed. Whitespace-only input is returned with its underscores replaced.
fn web_sanitize(taxon: &str) -> String {
    let replaced = taxon.replace('_', " ");
    let mut words = replaced.split_whitespace();
    let Some(first) = words.next() else {
        return replaced;
    };
    let mut chars = first.chars();
    let mut out = chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
    });
    for word in words {
        out.push(' ');
        out.push_str(word);
    }
    out
}

#[test]
fn sanitising_underscores_and_title_cases_only_the_genus() {
    assert_eq!(web_sanitize("gentiana_lutea"), "Gentiana lutea");
    assert_eq!(web_sanitize("gentiana lutea"), "Gentiana lutea");
    assert_eq!(web_sanitize("VOACANGA africana"), "Voacanga africana");
    assert_eq!(web_sanitize("__gentiana_lutea__"), "Gentiana lutea");
    assert_eq!(web_sanitize("Gentiana Lutea"), "Gentiana Lutea");
    // Whitespace-only input has no first word, so the replaced string is
    // returned unchanged rather than being emptied.
    assert_eq!(web_sanitize("   "), "   ");
    assert_eq!(web_sanitize("__"), "  ");
}
