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
        route.clone().with_view("curation").navigation_string(),
        "/curation"
    );
    assert_eq!(route.with_view("draw").navigation_string(), "/draw");
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
fn root_and_search_are_distinct_routes() {
    let landing = "/".parse::<Route>();
    let search = "/search".parse::<Route>();
    assert!(landing.is_ok(), "landing route should parse");
    assert!(search.is_ok(), "search route should parse");
    if let (Ok(landing), Ok(search)) = (landing, search) {
        assert_eq!(landing.view_key(), "landing");
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
