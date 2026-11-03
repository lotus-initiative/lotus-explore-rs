// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use lotus_search::SearchCriteria;

/// Build a shareable `/search?...` URL for `criteria`, or `None` when the
/// criteria are all defaults.
#[must_use]
pub fn build_shareable_url(criteria: &SearchCriteria, year: u16) -> Option<String> {
    let params = super::criteria_query_params(criteria, year);
    if params.is_empty() {
        return None;
    }
    let mut out = String::from("/search?");
    append_query_pairs(
        &mut out,
        params.iter().map(|(k, v)| (k.as_str(), v.as_str())),
    );
    Some(out)
}

/// Encode `params` into a percent-encoded `key=value&...` query string.
#[must_use]
pub fn encode_query_params(params: &super::QueryParams) -> String {
    let mut query = String::new();
    append_query_pairs(
        &mut query,
        params.iter().map(|(k, v)| (k.as_str(), v.as_str())),
    );
    query
}

/// Decode a percent-encoded `?key=value&...` query string into [`super::QueryParams`].
#[must_use]
pub fn parse_query_string(query: &str) -> super::QueryParams {
    let mut out = super::QueryParams::new();
    for pair in query.trim_start_matches('?').split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, val) = pair.split_once('=').unwrap_or((pair, ""));
        let key_decoded = lotus_model::percent_decode(key)
            .map_or_else(|_| key.into(), std::borrow::Cow::into_owned);
        let val_decoded = lotus_model::percent_decode(val)
            .map_or_else(|_| val.into(), std::borrow::Cow::into_owned);
        out.insert(key_decoded, val_decoded);
    }
    out
}

fn append_query_pairs<'a>(out: &mut String, iter: impl Iterator<Item = (&'a str, &'a str)>) {
    for (index, (key, value)) in iter.enumerate() {
        if index > 0 {
            out.push('&');
        }
        let key_encoded = lotus_model::percent_encode(key);
        let value_encoded = lotus_model::percent_encode(value);
        out.push_str(&key_encoded);
        out.push('=');
        out.push_str(&value_encoded);
    }
}
