// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! The tests for `routes`, in their own file.

use super::{Route, RouteQuery, without_empty_url_delimiters};
use crate::i18n::Locale;

#[test]
fn routes_round_trip_query_and_hash_segments() {
    let parsed = "/curation?lang=fr&dark_mode=true#main-panel".parse::<Route>();
    assert!(parsed.is_ok(), "route should parse");
    if let Ok(route) = parsed {
        assert_eq!(route.view_key(), "curation");
        assert_eq!(route.query_string(), "dark_mode=true&lang=fr");
        assert_eq!(route.hash(), "main-panel");
        assert_eq!(
            route.to_string(),
            "/curation?dark_mode=true&lang=fr#main-panel"
        );
    }
}

#[test]
fn empty_url_delimiters_are_removed_without_losing_values() {
    assert_eq!(
        without_empty_url_delimiters("https://example.test/draw?"),
        Some("https://example.test/draw".to_string())
    );
    assert_eq!(
        without_empty_url_delimiters("https://example.test/draw#"),
        Some("https://example.test/draw".to_string())
    );
    assert_eq!(
        without_empty_url_delimiters("https://example.test/draw?#"),
        Some("https://example.test/draw".to_string())
    );
    assert_eq!(
        without_empty_url_delimiters("https://example.test/draw?dark_mode=true#editor"),
        None
    );
    assert_eq!(
        without_empty_url_delimiters("https://example.test/draw"),
        None
    );
}

#[test]
fn navigation_strings_omit_empty_query_delimiters() {
    let route = Route::Landing {
        query: RouteQuery::default(),
        hash: String::new(),
    };
    assert_eq!(route.navigation_string(), "/");
    let search = route.clone().with_view("search");
    assert_ne!(search, route);
    assert_eq!(search.navigation_string(), "/search");
    assert_eq!(
        route.clone().with_view("network").navigation_string(),
        "/network"
    );
    assert_eq!(
        route.clone().with_view("curation").navigation_string(),
        "/curation"
    );
    assert_eq!(route.with_view("draw").navigation_string(), "/draw");
}

#[test]
fn the_network_route_parses_and_preserves_preferences() {
    let parsed =
        "/network?taxon=Gentiana&execute=true&lang=fr&dark_mode=true#graph".parse::<Route>();
    assert!(parsed.is_ok(), "network route should parse");
    if let Ok(route) = parsed {
        assert_eq!(route.view_key(), "network");
        assert_eq!(route.hash(), "graph");
        let back = route.with_view("search");
        assert_eq!(back.view_key(), "search");
        assert_eq!(back.query_string(), "dark_mode=true&lang=fr");
        assert_eq!(back.hash(), "");
    }
}

#[test]
fn encoded_ampersands_in_json_query_are_preserved() {
    let route = Route::Curation {
        query: RouteQuery::from_encoded("curation_rows=%5B%7B%22name%22%3A%22A%26B%22%7D%5D"),
        hash: String::new(),
    };
    assert_eq!(
        route.to_string(),
        "/curation?curation_rows=%5B%7B%22name%22%3A%22A%26B%22%7D%5D"
    );
}

#[test]
fn parsed_json_query_preserves_encoded_ampersands() {
    let parsed = "/curation?curation_rows=%5B%7B%22name%22%3A%22A%26B%22%7D%5D".parse::<Route>();
    assert!(parsed.is_ok(), "route should parse");
    if let Ok(route) = parsed {
        assert_eq!(
            route.query_string(),
            "curation_rows=%5B%7B%22name%22%3A%22A%26B%22%7D%5D"
        );
    }
}

#[test]
fn unmatched_quote_or_paren_in_plain_param_does_not_swallow_next_param() {
    let q = RouteQuery::from_decoded("structure=C1(C&lang=fr&taxon=a\"b&dark_mode=true");
    assert_eq!(q.get("structure"), Some("C1(C"));
    assert_eq!(q.get("lang"), Some("fr"));
    assert_eq!(q.get("taxon"), Some("a\"b"));
    assert_eq!(q.get("dark_mode"), Some("true"));
}

#[test]
fn root_and_search_are_distinct_routes_showing_the_same_view() {
    let landing = "/".parse::<Route>();
    let search = "/search".parse::<Route>();
    assert!(landing.is_ok(), "landing route should parse");
    assert!(search.is_ok(), "search route should parse");
    if let (Ok(landing), Ok(search)) = (landing, search) {
        assert_ne!(landing, search);
        assert_eq!(landing.view_key(), "search");
        assert_eq!(search.view_key(), "search");
    }
}

#[test]
fn unknown_paths_use_the_not_found_route() {
    let route = "/missing/page".parse::<Route>();
    assert!(route.is_ok(), "unknown route should parse");
    if let Ok(route) = route {
        assert_eq!(route.view_key(), "not-found");
    }
}

#[test]
fn changing_view_resets_page_query_and_hash() {
    let route = Route::Search {
        query: RouteQuery::from_encoded("taxon=Rosa&lang=fr"),
        hash: "results".into(),
    };
    let route = route.with_view("draw");
    assert_eq!(route.view_key(), "draw");
    assert_eq!(route.query_string(), "lang=fr");
    assert_eq!(route.hash(), "");
}

#[test]
fn the_faq_route_parses_and_keeps_its_fragment() {
    // The fragment is the whole point of a per-question anchor: `/faq#taxon-not-found`
    // has to survive routing, or every deep link into a question opens the top of
    // the page.
    let parsed = "/faq#taxon-not-found".parse::<Route>();
    assert!(parsed.is_ok(), "faq route should parse");
    if let Ok(route) = parsed {
        assert_eq!(route.view_key(), "faq");
        assert_eq!(route.hash(), "taxon-not-found");
        assert_eq!(route.navigation_string(), "/faq#taxon-not-found");
    }
}

#[test]
fn the_faq_route_carries_the_preferences_that_survive_a_view_change() {
    // Language and dark mode live in the query, so navigating to the FAQ and back
    // must not drop them. `page_query` is what keeps them, which is why this goes
    // through `with_view` rather than constructing the route.
    let route = Route::Faq {
        query: RouteQuery::from_encoded("lang=fr&dark_mode=true"),
        hash: String::new(),
    };
    assert_eq!(route.navigation_string(), "/faq?dark_mode=true&lang=fr");

    let back_to_search = route.with_view("search");
    assert_eq!(back_to_search.view_key(), "search");
    assert_eq!(
        back_to_search.query_string(),
        "dark_mode=true&lang=fr",
        "leaving the FAQ for search must keep the locale and theme"
    );
}

#[test]
fn changing_view_onto_the_faq_resets_page_query_and_hash() {
    // The mirror image: a query written *for* another page must not follow the
    // reader to the FAQ, where it means nothing and would sit in the URL.
    let route = Route::Search {
        query: RouteQuery::from_encoded("taxon=Rosa&lang=fr"),
        hash: "results".into(),
    }
    .with_view("faq");
    assert_eq!(route.view_key(), "faq");
    assert_eq!(route.query_string(), "lang=fr");
    assert_eq!(route.hash(), "");
}

#[test]
fn preference_updates_preserve_route_metadata() {
    let route = Route::Draw {
        query: RouteQuery::from_encoded("api_base=https%3A%2F%2Fexample.org"),
        hash: "main-panel".into(),
    }
    .with_locale(Locale::Fr)
    .with_dark_mode(true);
    assert_eq!(
        route.query_string(),
        "api_base=https%3A%2F%2Fexample.org&dark_mode=true&lang=fr"
    );
    assert_eq!(route.hash(), "main-panel");
}

#[test]
fn every_route_has_a_unique_localized_document_title() {
    use std::collections::HashSet;

    let sample_routes = [
        "/",
        "/search",
        "/network",
        "/curation",
        "/draw",
        "/faq",
        "/does-not-exist",
    ];
    for locale in [Locale::En, Locale::Fr] {
        let mut seen = HashSet::new();
        for raw in sample_routes {
            if let Ok(route) = raw.parse::<Route>() {
                let title = route.document_title(locale);
                assert!(!title.trim().is_empty(), "empty title for {raw}");
                assert!(
                    seen.insert(title.clone()),
                    "duplicate title '{title}' for {raw} in {locale:?}"
                );
            }
        }
        assert_eq!(seen.len(), sample_routes.len());
    }
}

#[test]
fn with_network_query_preserves_preferences_and_replaces_stale_params() {
    use lotus_explore_core::explore::url_codec::QueryParams;

    let parsed = "/network?taxon=OldTaxon&focus=compound-Q1&max_neighbors=50&lang=fr&dark_mode=true&api_base=https%3A%2F%2Fexample.org#graph"
        .parse::<Route>();
    assert!(parsed.is_ok(), "network route should parse");
    if let Ok(route) = parsed {
        let params = QueryParams::from([
            ("execute".to_owned(), "true".to_owned()),
            ("mode".to_owned(), "2d".to_owned()),
            ("taxon".to_owned(), "Gentiana".to_owned()),
        ]);
        let updated = route.with_network_query(&params);
        assert_eq!(updated.view_key(), "network");
        assert_eq!(updated.hash(), "graph");
        assert_eq!(
            updated.navigation_string(),
            "/network?api_base=https%3A%2F%2Fexample.org&dark_mode=true&execute=true&lang=fr&mode=2d&taxon=Gentiana#graph"
        );

        let cleared = updated.with_network_query(&QueryParams::new());
        assert_eq!(
            cleared.navigation_string(),
            "/network?api_base=https%3A%2F%2Fexample.org&dark_mode=true&lang=fr#graph"
        );
    }
}

#[test]
fn network_route_exposes_all_search_and_viz_params() {
    let parsed = "/network?taxon=Gentiana&execute=true&projection=tripartite&mode=2d&paused=true&focus=compound-Q100&max_neighbors=120&selected=taxon-Q2000"
        .parse::<Route>();
    assert!(parsed.is_ok(), "network route should parse");
    if let Ok(route) = parsed {
        let query = route.query_value();
        let params = query.params();
        assert_eq!(params.get("taxon").map(String::as_str), Some("Gentiana"));
        assert_eq!(params.get("execute").map(String::as_str), Some("true"));
        assert_eq!(
            params.get("projection").map(String::as_str),
            Some("tripartite")
        );
        assert_eq!(params.get("mode").map(String::as_str), Some("2d"));
        assert_eq!(params.get("paused").map(String::as_str), Some("true"));
        assert_eq!(
            params.get("focus").map(String::as_str),
            Some("compound-Q100")
        );
        assert_eq!(params.get("max_neighbors").map(String::as_str), Some("120"));
        assert_eq!(
            params.get("selected").map(String::as_str),
            Some("taxon-Q2000")
        );
    }
}
