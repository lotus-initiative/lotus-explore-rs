// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use lotus_search::SearchCriteria;
pub fn build_shareable_url(criteria: &SearchCriteria) -> Option<String> {
    let params = super::criteria_query_params(criteria, crate::clock::current_year());
    if params.is_empty() {
        return None;
    }
    let query = build_query_string_from_pairs(params.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    let mut out = String::with_capacity(query.len() + 8);
    out.push_str("/search?");

    out.push_str(&query);
    Some(out)
}

fn build_query_string_from_pairs<'a>(iter: impl Iterator<Item = (&'a str, &'a str)>) -> String {
    let mut query = String::new();
    for (index, (key, value)) in iter.enumerate() {
        if index > 0 {
            query.push('&');
        }
        let key_encoded = urlencoding::encode(key);
        let value_encoded = urlencoding::encode(value);
        query.push_str(&key_encoded);
        query.push('=');
        query.push_str(&value_encoded);
    }
    query
}

#[cfg(test)]
pub(super) fn build_query_string_for_tests(params: &super::QueryParams) -> String {
    build_query_string_from_pairs(params.iter().map(|(k, v)| (k.as_str(), v.as_str())))
}
