// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use super::shell::{AppShell, ExplorePage};
use crate::components::data_curation_page::DataCurationPage;
use crate::components::landing::{LandingPage, NotFoundPage};
use crate::i18n::Locale;
use crate::pages::DrawPage;
use dioxus::prelude::*;
use dioxus::router::routable::FromQuery;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RouteQuery {
    params: BTreeMap<String, String>,
}

impl RouteQuery {
    /// Read a query string that is still percent-encoded.
    ///
    /// The router hands over a query it has already decoded, so `from_decoded` is
    /// what runs. This exists for a caller holding a raw `location.search`, and
    /// nothing does -- the browser reads the URL through the router.
    /// Present in a wasm *test* build as well as native: the tests below exercise the
    /// decoder directly, and gating it out of wasm is what stopped the wasm test target
    /// compiling at all.
    #[cfg(any(not(target_arch = "wasm32"), test))]
    pub fn from_encoded(query: &str) -> Self {
        Self {
            params: parse_encoded_query(query),
        }
    }

    fn from_decoded(query: &str) -> Self {
        Self {
            params: parse_decoded_query(query),
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(String::as_str)
    }

    fn set(&mut self, key: &str, value: &str) {
        self.params.insert(key.to_string(), value.to_string());
    }

    fn remove(&mut self, key: &str) {
        self.params.remove(key);
    }
}

impl FromQuery for RouteQuery {
    fn from_query(query: &str) -> Self {
        Self::from_decoded(query)
    }
}

impl fmt::Display for RouteQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&encode_query(&self.params))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Routable)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppShell)]
    #[route("/?:..query#:hash")]
    Landing { query: RouteQuery, hash: String },

    #[route("/search?:..query#:hash")]
    Search { query: RouteQuery, hash: String },

    #[route("/curation?:..query#:hash")]
    Curation { query: RouteQuery, hash: String },

    #[route("/draw?:..query#:hash")]
    Draw { query: RouteQuery, hash: String },

    #[route("/faq?:..query#:hash")]
    Faq { query: RouteQuery, hash: String },

    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

impl Route {
    // Only the tests below read this, so it is not compiled into a binary that
    // has no tests to run.
    #[cfg(test)]
    pub fn query_string(&self) -> String {
        self.query_value().to_string()
    }

    pub(crate) fn query_value(&self) -> RouteQuery {
        match self {
            Self::Landing { query, .. }
            | Self::Search { query, .. }
            | Self::Curation { query, .. }
            | Self::Draw { query, .. }
            | Self::Faq { query, .. } => query.clone(),
            Self::NotFound { .. } => RouteQuery::default(),
        }
    }

    pub fn navigation_string(&self) -> String {
        let path = match self {
            Self::Landing { .. } => "/".to_string(),
            Self::Search { .. } => "/search".to_string(),
            Self::Curation { .. } => "/curation".to_string(),
            Self::Draw { .. } => "/draw".to_string(),
            Self::Faq { .. } => "/faq".to_string(),
            Self::NotFound { segments } => format!("/{}", segments.join("/")),
        };
        let query = self.query_value().to_string();
        let hash = self.hash();
        let mut url = path;
        if !query.is_empty() {
            url.push('?');
            url.push_str(&query);
        }
        if !hash.is_empty() {
            url.push('#');
            url.push_str(hash);
        }
        url
    }

    pub fn hash(&self) -> &str {
        match self {
            Self::Landing { hash, .. }
            | Self::Search { hash, .. }
            | Self::Curation { hash, .. }
            | Self::Draw { hash, .. }
            | Self::Faq { hash, .. } => hash,
            Self::NotFound { .. } => "",
        }
    }

    pub fn view_key(&self) -> &'static str {
        match self {
            Self::Landing { .. } => "landing",
            Self::Search { .. } => "search",
            Self::Curation { .. } => "curation",
            Self::Draw { .. } => "draw",
            Self::Faq { .. } => "faq",
            Self::NotFound { .. } => "not-found",
        }
    }

    fn page_query(&self) -> RouteQuery {
        let current = self.query_value();
        let mut query = RouteQuery::default();
        for key in ["lang", "dark_mode", "api_base"] {
            if let Some(value) = current.get(key) {
                query.set(key, value);
            }
        }
        query
    }

    pub fn with_view(self, view: &str) -> Self {
        let query = self.page_query();
        let hash = String::new();
        match view {
            "search" => Self::Search { query, hash },
            "curation" => Self::Curation { query, hash },
            "draw" => Self::Draw { query, hash },
            "faq" => Self::Faq { query, hash },
            _ => Self::Landing { query, hash },
        }
    }

    pub fn with_locale(self, locale: Locale) -> Self {
        let mut query = self.query_value();
        if locale == Locale::En {
            query.remove("lang");
        } else {
            query.set("lang", locale.lang_code());
        }
        self.with_query_and_hash(query)
    }

    pub fn with_dark_mode(self, dark_mode: bool) -> Self {
        let mut query = self.query_value();
        if dark_mode {
            query.set("dark_mode", "true");
        } else {
            query.remove("dark_mode");
        }
        self.with_query_and_hash(query)
    }

    fn with_query_and_hash(self, query: RouteQuery) -> Self {
        let hash = self.hash().to_string();
        match self {
            Self::Landing { .. } => Self::Landing { query, hash },
            Self::Search { .. } => Self::Search { query, hash },
            Self::Curation { .. } => Self::Curation { query, hash },
            Self::Draw { .. } => Self::Draw { query, hash },
            Self::Faq { .. } => Self::Faq { query, hash },
            Self::NotFound { segments } => Self::NotFound { segments },
        }
    }
}

pub fn normalize_empty_query() {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(window) = web_sys::window() else {
            return;
        };
        let location = window.location();
        let Ok(href) = location.href() else {
            return;
        };
        let Some(clean) = without_empty_url_delimiters(&href) else {
            return;
        };
        if let Ok(history) = window.history() {
            let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&clean));
        }
    }
}

fn without_empty_url_delimiters(href: &str) -> Option<String> {
    let (without_hash, hash) = href
        .split_once('#')
        .map_or((href, None), |(path, hash)| (path, Some(hash)));
    let (path, query) = without_hash
        .split_once('?')
        .map_or((without_hash, None), |(path, query)| (path, Some(query)));

    let mut clean = String::with_capacity(href.len());
    clean.push_str(path);
    if let Some(query) = query.filter(|query| !query.is_empty()) {
        clean.push('?');
        clean.push_str(query);
    }
    if let Some(hash) = hash.filter(|hash| !hash.is_empty()) {
        clean.push('#');
        clean.push_str(hash);
    }
    (clean != href).then_some(clean)
}

#[component]
pub fn Landing(query: RouteQuery, hash: String) -> Element {
    let _ = (query, hash);
    rsx! { LandingPage {} }
}

#[component]
pub fn Search(query: RouteQuery, hash: String) -> Element {
    let _ = (query, hash);
    rsx! { ExplorePage {} }
}

#[component]
pub fn Curation(query: RouteQuery, hash: String) -> Element {
    let _ = (query, hash);
    rsx! { DataCurationPage {} }
}

#[component]
pub fn Draw(query: RouteQuery, hash: String) -> Element {
    let _ = (query, hash);
    rsx! { DrawPage {} }
}

#[component]
pub fn Faq(query: RouteQuery, hash: String) -> Element {
    rsx! { crate::components::faq::FaqPage { query, hash } }
}

#[component]
pub fn NotFound(segments: Vec<String>) -> Element {
    let _ = segments;
    rsx! { NotFoundPage {} }
}

// Only the native and server paths reach this; the browser client has its
// own fetch path, so a wasm build has no caller for it.
#[cfg(any(not(target_arch = "wasm32"), test))]
fn parse_encoded_query(query: &str) -> BTreeMap<String, String> {
    query
        .trim_start_matches('?')
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode_component(key), decode_component(value))
        })
        .collect()
}

fn parse_decoded_query(query: &str) -> BTreeMap<String, String> {
    split_decoded_query(query)
        .into_iter()
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            if key.is_empty() {
                return None;
            }
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

fn split_decoded_query(query: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in query.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '[' | '{' | '(' => depth = depth.saturating_add(1),
            ']' | '}' | ')' => depth = depth.saturating_sub(1),
            '&' if depth == 0 => {
                parts.push(&query[start..index]);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&query[start..]);
    parts
}

// Only the native and server paths reach this; the browser client has its
// own fetch path, so a wasm build has no caller for it.
#[cfg(any(not(target_arch = "wasm32"), test))]
fn decode_component(value: &str) -> String {
    urlencoding::decode(value).map_or_else(|_| value.to_string(), std::borrow::Cow::into_owned)
}

fn encode_query(params: &BTreeMap<String, String>) -> String {
    params
        .iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                urlencoding::encode(key),
                urlencoding::encode(value)
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

#[cfg(test)]
#[path = "routes/tests.rs"]
mod tests;
