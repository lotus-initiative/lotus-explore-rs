// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

#![allow(clippy::expect_used, clippy::float_cmp)]

use super::*;
use lotus_model::{ElementState, SearchCriteria, SmilesSearchType};
use lotus_query::ExportFormat as DownloadFormat;

#[test]
fn parse_criteria_supports_formula_and_halogens() {
    let mut params = QueryParams::new();
    params.insert("taxon".into(), "*".into());
    params.insert("formula_filter".into(), "true".into());
    params.insert("c_min".into(), "15".into());
    params.insert("c_max".into(), "25".into());
    params.insert("o_min".into(), "2".into());
    params.insert("o_max".into(), "8".into());
    params.insert("f_state".into(), "required".into());
    params.insert("cl_state".into(), "required".into());
    params.insert("br_state".into(), "excluded".into());
    params.insert("i_state".into(), "excluded".into());

    let crit = parse_criteria_from_params(&params, 2026);
    assert!(crit.formula_enabled);
    assert_eq!(crit.c_min, 15);
    assert_eq!(crit.c_max, 25);
    assert_eq!(crit.o_min, 2);
    assert_eq!(crit.o_max, 8);
    assert_eq!(crit.f_state, ElementState::Required);
    assert_eq!(crit.cl_state, ElementState::Required);
    assert_eq!(crit.br_state, ElementState::Excluded);
    assert_eq!(crit.i_state, ElementState::Excluded);
}

#[test]
fn parse_criteria_structure_without_explicit_taxon_clears_default_taxon() {
    let mut params = QueryParams::new();
    params.insert("structure".into(), "CCO".into());

    let crit = parse_criteria_from_params(&params, 2026);
    assert_eq!(crit.structure, "CCO");
    assert_eq!(crit.taxon.len(), 0, "expected no entries");
}

#[test]
fn startup_action_execute_only() {
    let mut params = QueryParams::new();
    params.insert("execute".into(), "true".into());
    let startup = parse_startup_action_from_params(&params);
    assert!(startup.pending_format.is_none());
    assert!(startup.pending_invalid_format.is_none());
    assert!(startup.direct_execute);
}

#[test]
fn share_params_roundtrip_for_advanced_filters() {
    let mut crit = SearchCriteria {
        taxon: "*".into(),
        ..SearchCriteria::up_to_year(2026)
    };
    crit.formula_enabled = true;
    crit.c_min = 15;
    crit.c_max = 25;
    crit.o_min = 2;
    crit.o_max = 8;
    crit.f_state = ElementState::Required;
    crit.cl_state = ElementState::Required;
    crit.br_state = ElementState::Excluded;
    crit.i_state = ElementState::Excluded;

    let params = super::criteria_query_params(&crit, 2026);
    let reparsed = parse_criteria_from_params(&params, 2026);
    assert_eq!(reparsed.taxon, crit.taxon);
    assert_eq!(reparsed.c_min, crit.c_min);
    assert_eq!(reparsed.c_max, crit.c_max);
    assert_eq!(reparsed.o_min, crit.o_min);
    assert_eq!(reparsed.o_max, crit.o_max);
    assert_eq!(reparsed.f_state, crit.f_state);
    assert_eq!(reparsed.cl_state, crit.cl_state);
    assert_eq!(reparsed.br_state, crit.br_state);
    assert_eq!(reparsed.i_state, crit.i_state);
}

#[test]
fn share_params_keep_formula_toggle_but_omit_default_formula_bounds() {
    let crit = SearchCriteria {
        taxon: "Fungi".into(),
        formula_enabled: true,
        ..SearchCriteria::up_to_year(2026)
    };

    let params = super::criteria_query_params(&crit, 2026);
    let reparsed = parse_criteria_from_params(&params, 2026);

    assert_eq!(params.get("taxon").map(String::as_str), Some("Fungi"));
    assert_eq!(
        params.get("formula_filter").map(String::as_str),
        Some("true")
    );
    assert!(!params.contains_key("c_min"));
    assert!(!params.contains_key("c_max"));
    assert!(!params.contains_key("cl_state"));
    assert!(reparsed.formula_enabled);
    assert_eq!(reparsed.c_min, SearchCriteria::up_to_year(2026).c_min);
    assert_eq!(reparsed.c_max, SearchCriteria::up_to_year(2026).c_max);
    assert_eq!(reparsed.cl_state, SearchCriteria::up_to_year(2026).cl_state);
}

#[test]
fn startup_action_download_has_priority_over_execute() {
    let mut params = QueryParams::new();
    params.insert("download".into(), "yes".into());
    params.insert("execute".into(), "true".into());
    params.insert("format".into(), "rdf".into());
    let startup = parse_startup_action_from_params(&params);
    assert_eq!(startup.pending_format, Some(DownloadFormat::Rdf));
    assert!(startup.pending_invalid_format.is_none());
    assert!(!startup.direct_execute);
}

#[test]
fn startup_action_invalid_download_format_is_preserved() {
    let mut params = QueryParams::new();
    params.insert("download".into(), "1".into());
    params.insert("format".into(), "nt".into());

    let startup = parse_startup_action_from_params(&params);
    assert!(startup.pending_format.is_none());
    assert_eq!(startup.pending_invalid_format.as_deref(), Some("nt"));
    assert!(!startup.direct_execute);
}

#[test]
fn parse_criteria_rejects_non_positive_similarity_threshold() {
    let mut params = QueryParams::new();
    params.insert("similarity_threshold".into(), "0".into());

    let crit = parse_criteria_from_params(&params, 2026);
    assert_eq!(
        crit.structure_threshold,
        SearchCriteria::up_to_year(2026).structure_threshold
    );
}

#[test]
fn parse_criteria_clamps_low_positive_similarity_threshold() {
    let mut params = QueryParams::new();
    params.insert("similarity_threshold".into(), "0.01".into());

    let crit = parse_criteria_from_params(&params, 2026);
    assert_eq!(crit.structure_threshold, 0.05);
}

#[test]
fn shareable_search_urls_use_the_search_route() {
    // The taxon is set explicitly. It used to be the value baked into
    // `SearchCriteria`, so this test passed without ever saying what
    // it was about -- and a default criteria that quietly searches one species
    // is a trap for whoever calls `default()` next.
    let criteria = SearchCriteria {
        taxon: "Gentiana lutea".into(),
        ..SearchCriteria::up_to_year(2026)
    };
    assert_eq!(
        build_shareable_url(&criteria, 2026),
        Some("/search?taxon=Gentiana%20lutea".to_string())
    );
}

#[test]
fn a_shareable_link_for_a_structure_carries_the_matching_mode() {
    let criteria = SearchCriteria {
        structure: "C[C@H](O)CO".into(),
        structure_search: SmilesSearchType::Similarity,
        structure_threshold: 0.85,
        ..SearchCriteria::up_to_year(2026)
    };
    let url = build_shareable_url(&criteria, 2026).expect("a structure is a search");
    assert!(url.contains("structure=C%5BC%40H%5D%28O%29CO"), "{url}");
    assert!(url.contains("structure_search_type=similarity"), "{url}");
    assert!(url.contains("similarity_threshold=0.85"), "{url}");
}

#[test]
fn a_named_compound_carries_the_mode_too() {
    // The mode applies to every input kind, not only to structures: a reader who
    // asks for compounds containing amarogentin gets them, and the link has to
    // say so or the search it reproduces comes back narrower than the one shared.
    let criteria = SearchCriteria {
        structure: "amarogentina".into(),
        structure_search: SmilesSearchType::Similarity,
        structure_threshold: 1.0,
        ..SearchCriteria::up_to_year(2026)
    };
    let url = build_shareable_url(&criteria, 2026).expect("a name is a search");
    assert!(url.contains("structure=amarogentina"), "{url}");
    assert!(url.contains("structure_search_type=similarity"), "{url}");
    assert!(url.contains("similarity_threshold=1.00"), "{url}");
}

#[test]
fn a_shareable_link_records_the_exact_mode_rather_than_omitting_it() {
    // Exact is the default, so leaving it out would work — until the day it does
    // not, and a link for a substructure search silently comes back as one row.
    let criteria = SearchCriteria {
        structure: "Q23118".into(),
        structure_search: SmilesSearchType::Exact,
        ..SearchCriteria::up_to_year(2026)
    };
    let url = build_shareable_url(&criteria, 2026).expect("a QID is a search");
    assert!(url.contains("structure=Q23118"), "{url}");
    assert!(url.contains("structure_search_type=exact"), "{url}");
    assert!(
        !url.contains("similarity_threshold"),
        "exact has no cutoff: {url}"
    );
}

#[test]
fn the_matching_mode_survives_a_named_compound_in_the_criteria() {
    // Hidden is not discarded. Putting a structure back into the field should
    // find the mode the reader had chosen, not the default.
    let criteria = SearchCriteria {
        structure: "amarogentina".into(),
        structure_search: SmilesSearchType::Substructure,
        structure_threshold: 0.7,
        ..SearchCriteria::up_to_year(2026)
    };
    assert_eq!(criteria.structure_search, SmilesSearchType::Substructure);
    assert!((criteria.structure_threshold - 0.7).abs() < f64::EPSILON);
}

#[test]
fn a_reference_round_trips_through_a_shareable_link_as_typed() {
    // The link carries the DOI, not the QID it resolves to. The reader shared what
    // they typed, and the lookup is the same either way.
    let criteria = SearchCriteria {
        reference: "10.1002/andp.18280880206".to_owned(),
        ..SearchCriteria::up_to_year(2026)
    };
    let url = build_shareable_url(&criteria, 2026).expect("a reference is a search");
    assert!(
        url.contains("reference=10.1002%2Fandp.18280880206"),
        "{url}"
    );

    // Decoded the way the browser hands them over, so this exercises the same
    // round trip a pasted link does rather than comparing raw text.
    let params = parse_query_string(url.trim_start_matches("/search"));
    let parsed = parse_criteria_from_params(&params, 2026);
    assert_eq!(parsed.reference, "10.1002/andp.18280880206");
}

#[test]
fn no_reference_in_the_link_means_no_reference_constraint() {
    let params = QueryParams::new();
    let parsed = parse_criteria_from_params(&params, 2026);
    assert!(
        parsed.reference.is_empty(),
        "a link written before the field existed must not arrive with one"
    );
}

#[test]
fn criteria_with_nothing_set_have_no_shareable_url() {
    // A URL that encodes no search is not a search, and offering one would put
    // an empty query string in someone's address bar.
    let empty = SearchCriteria::up_to_year(2026);
    assert!(build_shareable_url(&empty, 2026).is_none());
}

#[test]
fn build_shareable_url_encodes_query_pairs() {
    let mut params = QueryParams::new();
    params.insert("taxon name".into(), "Gentiana lutea".into());
    params.insert("structure".into(), "C=C".into());

    let query = encode_query_params(&params);
    assert!(query.contains("taxon%20name=Gentiana%20lutea"));
    assert!(query.contains("structure=C%3DC"));
    assert!(query.contains('&'));
    assert_eq!(parse_query_string(&format!("?{query}&")), params);
    assert_eq!(
        parse_query_string("?flag&key=value"),
        QueryParams::from([
            ("flag".into(), String::new()),
            ("key".into(), "value".into()),
        ])
    );
}

#[test]
fn only_the_recognised_spellings_are_true_flags() {
    for value in ["1", "true", " TRUE ", "yes", "On"] {
        assert!(is_true_flag(value), "{value}");
    }
    for value in ["0", "false", "no", "off", "", "2"] {
        assert!(!is_true_flag(value), "{value}");
    }
}

/// Every parameter family at once, written out and read back: the exact pairs
/// a shared link carries, and the criteria it reproduces.
#[test]
fn every_parameter_round_trips_through_a_shareable_link() {
    let crit = SearchCriteria {
        taxon: "Fungi".into(),
        reference: "10.1000/xyz".into(),
        taxon_names: lotus_model::TaxonNomenclature {
            basionyms: false,
            protonyms: false,
            ..lotus_model::TaxonNomenclature::ALL_ON
        },
        structure: "CCO".into(),
        structure_search: SmilesSearchType::Similarity,
        structure_threshold: 0.75,
        mass_min: 100.5,
        mass_max: 200.25,
        year_min: 2000,
        year_max: 2020,
        formula_enabled: true,
        formula_exact: "C2H6O".into(),
        n_max: 3,
        ..SearchCriteria::up_to_year(2026)
    };

    let params = criteria_query_params(&crit, 2026);
    let expected: QueryParams = [
        ("formula_exact", "C2H6O"),
        ("formula_filter", "true"),
        ("mass_filter", "true"),
        ("mass_max", "200.25"),
        ("mass_min", "100.5"),
        ("n_max", "3"),
        ("reference", "10.1000/xyz"),
        ("similarity_threshold", "0.75"),
        ("structure", "CCO"),
        ("structure_search_type", "similarity"),
        ("taxon", "Fungi"),
        ("taxon_basionyms", "false"),
        ("taxon_protonyms", "false"),
        ("year_end", "2020"),
        ("year_filter", "true"),
        ("year_start", "2000"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    assert_eq!(params, expected);

    assert_eq!(parse_criteria_from_params(&params, 2026), crit);
}

#[test]
fn a_blank_exact_formula_is_left_out_of_the_link() {
    let crit = SearchCriteria {
        taxon: "Fungi".into(),
        formula_enabled: true,
        formula_exact: "   ".into(),
        ..SearchCriteria::up_to_year(2026)
    };
    assert!(!criteria_query_params(&crit, 2026).contains_key("formula_exact"));
}

#[test]
fn each_search_type_spelling_parses_to_its_mode() {
    for (spelling, mode) in [
        ("exact", SmilesSearchType::Exact),
        ("substructure", SmilesSearchType::Substructure),
        ("similarity", SmilesSearchType::Similarity),
    ] {
        let params = QueryParams::from([("structure_search_type".to_owned(), spelling.to_owned())]);
        assert_eq!(
            parse_criteria_from_params(&params, 2026).structure_search,
            mode
        );
    }
}
