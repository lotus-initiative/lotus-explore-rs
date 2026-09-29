// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::i18n::Locale;
use std::collections::BTreeMap;

pub use super::url_codec::{
    InitialUrlState, build_shareable_url, is_true_flag, parse_criteria_from_params,
    parse_startup_action_from_params,
};

#[cfg(test)]
pub use super::url_codec::InitialDownloadState;

pub fn initial_url_state() -> InitialUrlState {
    let params = read_url_query_params();
    InitialUrlState {
        criteria: parse_criteria_from_params(&params),
        locale: Locale::detect(params.get("lang").map_or("", String::as_str)),
        download: parse_startup_action_from_params(&params),
        dark_mode: params.get("dark_mode").is_some_and(|v| is_true_flag(v)),
    }
}

fn deployment_base_path(pathname: &str) -> String {
    let path = pathname.trim_end_matches('/');
    if path.is_empty() {
        return String::new();
    }
    for suffix in ["/search", "/curation", "/draw"] {
        if let Some(base) = path.strip_suffix(suffix) {
            return base.to_string();
        }
    }
    path.to_string()
}

#[cfg(target_arch = "wasm32")]
pub fn deployment_href(path: &str) -> String {
    let mut prefix = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
        .and_then(|element| element.get_attribute("data-lotus-base-path"))
        .map(|value| value.trim_end_matches('/').to_string())
        .unwrap_or_default();
    if prefix.is_empty() {
        let pathname = web_sys::window()
            .and_then(|window| window.location().pathname().ok())
            .unwrap_or_default();
        prefix = deployment_base_path(&pathname);
    }
    prefix.push_str(path);
    prefix
}

#[cfg(not(target_arch = "wasm32"))]
pub fn deployment_href(path: &str) -> String {
    path.to_string()
}

#[cfg(target_arch = "wasm32")]
fn current_search() -> String {
    web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
        .and_then(|element| element.get_attribute("data-lotus-query"))
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn current_search() -> String {
    String::new()
}

pub fn href_with_current_query(path: &str) -> String {
    let query = current_search();
    let query = query.strip_prefix('?').unwrap_or(&query);
    let path = if query.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{query}")
    };
    deployment_href(&path)
}

pub fn absolute_share_url(share: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some((origin, _pathname)) = origin_and_pathname() {
            return format!("{origin}{}", deployment_href(share));
        }
    }
    share.into()
}

pub fn absolute_current_url_with_query(query: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some((origin, pathname)) = origin_and_pathname() {
            return if query.is_empty() {
                format!("{origin}{pathname}")
            } else {
                format!("{origin}{pathname}?{query}")
            };
        }
    }
    if query.is_empty() {
        String::new()
    } else {
        format!("?{query}")
    }
}

/// The browser's `location.origin` + `location.pathname` (no query), when available.
/// Centralizes the `web_sys::window()` lookup shared by the `absolute_*` URL builders so they
/// don't each re-open the window.
#[cfg(target_arch = "wasm32")]
fn origin_and_pathname() -> Option<(String, String)> {
    let win = web_sys::window()?;
    let loc = win.location();
    Some((loc.origin().ok()?, loc.pathname().ok()?))
}

pub fn read_url_query_params() -> BTreeMap<String, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(window) = web_sys::window() else {
            return BTreeMap::new();
        };
        let Ok(search) = window.location().search() else {
            return BTreeMap::new();
        };
        parse_query_string(&search)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        BTreeMap::new()
    }
}

fn parse_query_string(query: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for pair in query.trim_start_matches('?').split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, val) = pair.split_once('=').unwrap_or((pair, ""));
        let key_decoded =
            urlencoding::decode(key).map_or_else(|_| key.into(), std::borrow::Cow::into_owned);
        let val_decoded =
            urlencoding::decode(val).map_or_else(|_| val.into(), std::borrow::Cow::into_owned);
        out.insert(key_decoded, val_decoded);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::deployment_base_path;

    #[test]
    fn deployment_base_path_preserves_repository_prefix() {
        assert_eq!(deployment_base_path("/"), "");
        assert_eq!(
            deployment_base_path("/lotus-explore-rs/"),
            "/lotus-explore-rs"
        );
        assert_eq!(
            deployment_base_path("/lotus-explore-rs/search"),
            "/lotus-explore-rs"
        );
        assert_eq!(
            deployment_base_path("/lotus-explore-rs/curation"),
            "/lotus-explore-rs"
        );

        assert_eq!(
            deployment_base_path("/lotus-explore-rs/draw"),
            "/lotus-explore-rs"
        );
    }
}
