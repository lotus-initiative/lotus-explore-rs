// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

#![allow(clippy::expect_used)]
#![allow(clippy::indexing_slicing)]

use crate::features::curation::state::page_controller::rows_to_tsv;
use crate::features::explore::search_state::ExploreState;
use crate::features::explore::selectors::toolbar_snapshot_from_result;
use crate::ui::{ContentPhase, LifecycleBooleans};
use lotus_query::ExportFormat as DownloadFormat;

fn is_supported_download_format(fmt: &str) -> bool {
    DownloadFormat::parse(fmt).is_some()
}

#[test]
fn supported_download_formats_include_documented_values() {
    assert!(is_supported_download_format("csv"));
    assert!(is_supported_download_format("json"));
    assert!(is_supported_download_format("ndjson"));
    assert!(is_supported_download_format("ttl"));
    // The pre-rename name, kept so a shared link still offers the right format.
    assert!(is_supported_download_format("rdf"));
    assert!(!is_supported_download_format("turtle"));
}

#[test]
fn supported_download_formats_allow_case_and_whitespace_variants() {
    assert!(is_supported_download_format(" CSV "));
    assert!(is_supported_download_format("Json"));
    assert!(is_supported_download_format("RDF"));
}

#[test]
fn integration_explore_snapshot_drives_loaded_phase_and_toolbar_data() {
    let mut explore = ExploreState::default();
    explore.lifecycle.searched_once = true;
    explore.result.sparql_query = Some("SELECT * WHERE { ?s ?p ?o }".into());
    explore.result.total_matches = Some(3);

    let snapshot = toolbar_snapshot_from_result(&explore.result);
    let phase = ContentPhase::from(LifecycleBooleans {
        loading: explore.lifecycle.loading,
        has_error: explore.lifecycle.error.is_some(),
        searched_once: explore.lifecycle.searched_once,
        download_only_mode: explore.lifecycle.download_only_mode,
        has_entries: true,
        criteria_dirty: false,
    });

    assert_eq!(snapshot.total_matches, Some(3));
    assert!(snapshot.sparql_query.is_some());
    assert_eq!(phase, ContentPhase::Loaded);
}

/// Editing a field without pressing Search must take the results away.
///
/// Changing a form field does not dispatch a search -- the user presses the
/// button -- so without this the table and the stats cards sit there describing
/// the *previous* criteria while the form shows the new ones. Two panels on
/// screen disagreeing about what is being searched is worse than either being
/// absent, and it reads as a bug in the search rather than a search that has not
/// been run yet.
#[test]
fn editing_the_form_takes_the_previous_results_away() {
    let phase = |criteria_dirty: bool| {
        ContentPhase::from(LifecycleBooleans {
            loading: false,
            has_error: false,
            searched_once: true,
            download_only_mode: false,
            has_entries: true,
            criteria_dirty,
        })
    };

    assert_eq!(phase(false), ContentPhase::Loaded);
    assert_eq!(phase(true), ContentPhase::Stale);
}

/// Loading outranks a dirty form.
///
/// A search already running is what the user is waiting for; telling them their
/// results are stale while the new ones are in flight would be two messages at
/// once, and the second would be wrong within a second.
#[test]
fn loading_outranks_a_dirty_form() {
    let phase = ContentPhase::from(LifecycleBooleans {
        loading: true,
        has_error: false,
        searched_once: true,
        download_only_mode: false,
        has_entries: true,
        criteria_dirty: true,
    });
    assert_eq!(phase, ContentPhase::Loading);
}

#[test]
fn integration_curation_rows_tsv_round_trip_keeps_expected_header() {
    let rows = vec![crate::curation::CurationInputRow {
        name: "A name".to_string(),
        smiles: "CCO".to_string(),
        taxon: Some("Rosa canina".to_string()),
        doi: Some("10.1000/ABC".to_string()),
    }];

    let tsv = rows_to_tsv(&rows);
    let parsed = crate::curation::parse_tsv_rows(&tsv).expect("tsv should parse");

    assert!(tsv.starts_with("name\tsmiles\ttaxon\tdoi\n"));
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].name, "A name");
    assert_eq!(parsed[0].smiles, "CCO");
}

/// The generated ARD capability manifest, as agents will fetch it from
/// `/.well-known/ai-catalog.json`.
mod ai_catalog {
    use serde_json::Value;

    const SPEC_VERSION: &str = "1.0";
    const ROOT_KEYS: [&str; 3] = ["specVersion", "host", "entries"];
    const HOST_KEYS: [&str; 5] = [
        "displayName",
        "identifier",
        "documentationUrl",
        "logoUrl",
        "trustManifest",
    ];
    const ENTRY_REQUIRED: [&str; 3] = ["identifier", "displayName", "type"];
    /// `^urn:air:[a-zA-Z0-9.-]+(:[a-zA-Z0-9._-]+)+$`
    const URN_NID: &str = "urn:air:";

    /// Read at run time, not with `include_str!`.
    fn catalog() -> Option<Value> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("public/.well-known/ai-catalog.json");
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    fn catalog_or_fail() -> Value {
        catalog().expect("ai-catalog.json must be present and valid JSON")
    }

    fn is_urn(s: &str) -> bool {
        let Some(rest) = s.strip_prefix(URN_NID) else {
            return false;
        };
        let parts: Vec<&str> = rest.split(':').collect();
        let nid_ok = parts.first().is_some_and(|nid| {
            !nid.is_empty()
                && nid
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        });
        let rest_ok = parts.len() >= 2
            && parts[1..].iter().all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
            });
        nid_ok && rest_ok
    }

    #[test]
    fn root_matches_the_schema_envelope() {
        let doc = catalog_or_fail();
        let obj = doc.as_object().expect("catalog root must be an object");
        for key in ROOT_KEYS {
            assert!(obj.contains_key(key), "root must have {key}");
        }
        // additionalProperties: false on the root.
        for key in obj.keys() {
            assert!(
                ROOT_KEYS.contains(&key.as_str()),
                "unexpected root key {key}"
            );
        }
        assert_eq!(doc["specVersion"], Value::from(SPEC_VERSION));
    }

    #[test]
    fn host_carries_only_allowed_keys() {
        let doc = catalog_or_fail();
        let host = doc["host"].as_object().expect("host must be an object");
        assert!(
            host.contains_key("displayName"),
            "host.displayName is required"
        );
        for key in host.keys() {
            assert!(
                HOST_KEYS.contains(&key.as_str()),
                "unexpected host key {key}"
            );
        }
    }

    #[test]
    fn entries_satisfy_the_required_fields_and_urn_grammar() {
        let doc = catalog_or_fail();
        let entries = doc["entries"].as_array().expect("entries must be an array");
        assert!(!entries.is_empty(), "a catalog with no entries is useless");
        for entry in entries {
            for key in ENTRY_REQUIRED {
                assert!(entry.get(key).is_some(), "entry must have {key}");
            }
            let identifier = entry["identifier"]
                .as_str()
                .expect("identifier is a string");
            assert!(
                is_urn(identifier),
                "identifier {identifier} is not a valid ARD URN"
            );
        }
    }

    #[test]
    fn representative_queries_stay_within_two_to_five() {
        let doc = catalog_or_fail();
        for entry in doc["entries"].as_array().expect("entries is an array") {
            let queries = entry["representativeQueries"]
                .as_array()
                .expect("representativeQueries is an array");
            assert!(
                (2..=5).contains(&queries.len()),
                "ARD wants 2-5 representative queries, found {}",
                queries.len()
            );
            for query in queries {
                assert!(query.as_str().is_some_and(|q| !q.trim().is_empty()));
            }
        }
    }

    #[test]
    fn every_entry_url_is_an_absolute_https_url() {
        let doc = catalog_or_fail();
        for entry in doc["entries"].as_array().expect("entries is an array") {
            let url = entry["url"].as_str().expect("url is a string");
            assert!(
                url.starts_with("https://"),
                "entry url must be https: {url}"
            );
        }
    }
}

/// The pre-stylesheet paint, which has to stand on its own.
mod boot_paint {
    const INDEX: &str = include_str!("../index.html");
    const STYLES: &str = include_str!("../tailwind/styles.css");

    /// Does `styles.css` declare `--name` with this value inside the dark block?
    fn dark_token(name: &str, value: &str) -> bool {
        let dark = STYLES
            .split_once("(prefers-color-scheme: dark)")
            .map_or(STYLES, |(_, rest)| rest);
        dark.lines().any(|line| {
            let line = line.trim();
            line.starts_with(&format!("--{name}:")) && line.contains(value)
        })
    }

    #[test]
    fn inline_style_has_a_dark_scheme_block() {
        assert!(
            INDEX.contains("@media (prefers-color-scheme: dark)"),
            "the pre-stylesheet paint must handle dark mode, or it paints light \
             backgrounds with dark text"
        );
    }

    #[test]
    fn inline_fallbacks_match_the_theme_tokens() {
        // The values the browser actually uses before the stylesheet arrives.
        for (name, value) in [("shell-page-bg", "#0a0f19"), ("text", "#eef4fb")] {
            assert!(
                dark_token(name, value),
                "--{name}: {value} must exist in the dark block of tailwind/styles.css \
                 and be mirrored in the inline style in index.html"
            );
            assert!(
                INDEX.contains(value),
                "index.html inline style is missing {value} for --{name}"
            );
        }
    }

    #[test]
    fn the_light_fallback_is_still_the_light_token() {
        assert!(INDEX.contains("#f2f5f8"), "light page fallback");
        assert!(dark_token("shell-page-bg", "#0a0f19"));
    }
}
