// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! `QLever` query timeout budgets, bearer authentication headers, and form body encoding.

use super::error_format::urlencode;

/// The header `QLever` reads an access token from.
///
/// `Authorization: Bearer <token>`, read by `ParsedRequestBuilder::addPrefixOrGet`
/// alongside an `access-token` parameter.
pub(super) const ACCESS_TOKEN_HEADER: &str = "Authorization";

/// The `QLever` instance's own limits, as constants rather than as numbers in
/// the middle of a function.
pub(super) struct QLever;

impl QLever {
    pub(super) const HOST: &'static str = "qlever.dev";
    const DEFAULT_QUERY_BUDGET: &'static str = "30s";
    pub(super) const MAX_QUERY_BUDGET: &'static str = "30s";
    pub(super) const CLIENT_ID: &'static str = crate::CLIENT_ID;
}

/// The form body for a POST, including the time budget when the endpoint has one.
pub(super) fn form_body(endpoint: &str, query: &str) -> String {
    query_budget(endpoint).map_or_else(
        || format!("query={}", urlencode(query)),
        |budget| {
            format!(
                "query={}&timeout={}",
                urlencode(query),
                urlencode(budget.as_str())
            )
        },
    )
}

/// The time budget to ask `endpoint` for, and to hold ourselves to.
#[must_use]
pub(super) fn query_budget(endpoint: &str) -> Option<String> {
    query_budget_from(endpoint, |name| std::env::var(name).ok())
}

/// Like [`query_budget`], but with the environment supplied by the caller.
pub(super) fn query_budget_from<F>(endpoint: &str, mut env: F) -> Option<String>
where
    F: FnMut(&str) -> Option<String>,
{
    if !endpoint.contains(QLever::HOST) {
        return None;
    }
    let authenticated = access_token_from(&mut env).is_some();
    Some(
        env("LOTUS_QLEVER_TIMEOUT")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .and_then(|value| {
                if authenticated {
                    parse_budget_millis(&value).map(|ms| format!("{ms}ms"))
                } else {
                    clamp_budget(&value)
                }
            })
            .unwrap_or_else(|| QLever::DEFAULT_QUERY_BUDGET.to_string()),
    )
}

/// Headers sent with every SPARQL POST request.
pub(super) fn request_headers(endpoint: &str) -> Vec<(&'static str, String)> {
    let mut headers = vec![("api-user-agent", QLever::CLIENT_ID.to_string())];
    if endpoint.contains(QLever::HOST)
        && let Some(token) = access_token()
    {
        headers.push((ACCESS_TOKEN_HEADER, format!("Bearer {token}")));
    }
    headers
}

pub(super) fn access_token() -> Option<String> {
    access_token_from(|name| std::env::var(name).ok())
}

pub(super) fn access_token_from<F>(mut env: F) -> Option<String>
where
    F: FnMut(&str) -> Option<String>,
{
    let token = env("LOTUS_QLEVER_TOKEN")?.trim().to_string();
    if token.is_empty() { None } else { Some(token) }
}

fn parse_budget_millis(value: &str) -> Option<u64> {
    let (digits, unit) = value.split_at(
        value
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(value.len()),
    );
    let amount: u64 = digits.parse().ok()?;
    if amount == 0 {
        return None;
    }
    match unit.trim() {
        "ms" => Some(amount),
        "s" | "sec" | "" => amount.checked_mul(1_000),
        "min" => amount.checked_mul(60_000),
        _ => None,
    }
}

pub(super) fn clamp_budget(value: &str) -> Option<String> {
    let millis = parse_budget_millis(value)?;
    let ceiling: u64 = QLever::MAX_QUERY_BUDGET
        .strip_suffix('s')
        .and_then(|seconds: &str| seconds.parse::<u64>().ok())
        .map_or(30_000, |seconds| seconds * 1_000);
    Some(format!("{}ms", millis.min(ceiling)))
}
