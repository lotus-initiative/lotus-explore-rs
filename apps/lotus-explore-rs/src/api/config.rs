// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

pub fn api_base_url() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(base) = runtime_query_param("api_base")
            && let Some(normalized) = normalize_api_base(&base)
        {
            return Some(normalized);
        }
    }

    if let Some(base) = option_env!("LOTUS_API_BASE")
        && let Some(normalized) = normalize_api_base(base)
    {
        return Some(normalized);
    }

    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
            && let Ok(hostname) = window.location().hostname()
        {
            let hostname = hostname.to_ascii_lowercase();
            if hostname == "localhost" || hostname == "127.0.0.1" {
                // Use relative/empty base for development to leverage Dioxus dev server proxy.
                // The dev server is configured to proxy /v1 requests to the API backend.
                return Some(String::new());
            }
        }
    }

    None
}

fn normalize_api_base(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return None;
    }
    if is_sparql_endpoint(trimmed) {
        return None;
    }
    Some(trimmed.to_string())
}

fn is_sparql_endpoint(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    value.contains("qlever") || value.contains("query.wikidata.org") || value.ends_with("/wikidata")
}

#[cfg(target_arch = "wasm32")]
fn runtime_query_param(name: &str) -> Option<String> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    let query = search.trim_start_matches('?');
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or_default();
        let value = parts.next().unwrap_or_default();
        let decoded_key = urlencoding::decode(key).ok()?;
        if decoded_key == name {
            return urlencoding::decode(value)
                .ok()
                .map(std::borrow::Cow::into_owned);
        }
    }
    None
}

#[cfg(test)]
#[path = "config/tests.rs"]
mod tests;
