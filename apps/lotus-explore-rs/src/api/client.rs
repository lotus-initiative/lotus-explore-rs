// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project

use crate::api::config::api_base_url;
use lotus_explore_core::api::dto::{SearchRequest, SearchResponse};
use lotus_explore_core::api::error::ApiClientError;
use lotus_model::SearchCriteria;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;

const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn search(
    criteria: &SearchCriteria,
    limit: usize,
    include_counts: bool,
) -> Result<SearchResponse, ApiClientError> {
    let base = api_base_url().unwrap_or_default();
    let request = SearchRequest::from_criteria(
        criteria,
        limit,
        include_counts,
        crate::clock::current_year(),
    );
    match post_json(&base, "/v1/search", &request).await {
        Ok(response) => Ok(response),
        Err(ApiClientError::Http(status, _))
            if include_counts && (status == 502 || status == 504) =>
        {
            log::warn!("event=search phase=api state=retry_without_counts status={status}");
            let retry_request =
                SearchRequest::from_criteria(criteria, limit, false, crate::clock::current_year());
            post_json(&base, "/v1/search", &retry_request).await
        }
        Err(err) => Err(err),
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn export_urls(
    criteria: &SearchCriteria,
) -> Result<lotus_explore_core::api::dto::ExportUrlResponse, ApiClientError> {
    let base = api_base_url().unwrap_or_default();
    let request = SearchRequest::from_criteria(criteria, 1, false, crate::clock::current_year());
    let response: lotus_explore_core::api::dto::ExportUrlResponse =
        post_json(&base, "/v1/export-url", &request).await?;
    Ok(normalize_export_urls(&base, response))
}

#[derive(Debug, thiserror::Error)]
#[error("failed to initialize HTTP client: {0}")]
struct ClientInitError(#[from] reqwest::Error);

fn http_client() -> Result<&'static reqwest::Client, ApiClientError> {
    static CLIENT: OnceLock<Result<reqwest::Client, ClientInitError>> = OnceLock::new();
    match CLIENT.get_or_init(build_http_client) {
        Ok(client) => Ok(client),
        Err(err) => Err(ApiClientError::Network(err.to_string())),
    }
}

fn build_http_client() -> Result<reqwest::Client, ClientInitError> {
    #[cfg(target_arch = "wasm32")]
    {
        Ok(reqwest::Client::builder().build()?)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Ok(reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(API_REQUEST_TIMEOUT)
            .pool_idle_timeout(Duration::from_mins(1))
            .pool_max_idle_per_host(8)
            .tcp_keepalive(Duration::from_secs(30))
            .build()?)
    }
}

#[cfg(target_arch = "wasm32")]
fn normalize_export_urls(
    base: &str,
    mut response: lotus_explore_core::api::dto::ExportUrlResponse,
) -> lotus_explore_core::api::dto::ExportUrlResponse {
    response.csv_url = resolve_api_url(base, &response.csv_url);
    response.json_url = resolve_api_url(base, &response.json_url);
    response.rdf_url = resolve_api_url(base, &response.rdf_url);
    response.csv_gz_url = response.csv_gz_url.map(|url| resolve_api_url(base, &url));
    response.json_gz_url = response.json_gz_url.map(|url| resolve_api_url(base, &url));
    response.rdf_gz_url = response.rdf_gz_url.map(|url| resolve_api_url(base, &url));
    response
}

#[cfg(target_arch = "wasm32")]
fn resolve_api_url(base: &str, url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }
    let base = base.trim_end_matches('/');
    if trimmed.starts_with('/') {
        format!("{base}{trimmed}")
    } else {
        format!("{base}/{trimmed}")
    }
}

#[cfg(target_arch = "wasm32")]
async fn with_api_timeout<T, E, F>(
    fut: F,
    map_err: impl FnOnce(E) -> ApiClientError,
) -> Result<T, ApiClientError>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    let timeout = lotus_search::reqwest_client::wait_deadline(API_REQUEST_TIMEOUT);
    futures::pin_mut!(fut);
    futures::pin_mut!(timeout);
    match futures::future::select(fut, timeout).await {
        futures::future::Either::Left((res, _timeout)) => res.map_err(map_err),
        futures::future::Either::Right(((), _fut)) => {
            Err(ApiClientError::Network("API request timed out".to_string()))
        }
    }
}

async fn post_json<Req, Res>(base: &str, path: &str, body: &Req) -> Result<Res, ApiClientError>
where
    Req: Serialize + ?Sized + Sync,
    Res: for<'de> Deserialize<'de>,
{
    let url = format!("{}{}", base.trim_end_matches('/'), path);
    let client = http_client()?;
    #[cfg(target_arch = "wasm32")]
    let response = with_api_timeout(client.post(url).json(body).send(), |e| {
        ApiClientError::Network(e.to_string())
    })
    .await?;
    #[cfg(not(target_arch = "wasm32"))]
    let response = client
        .post(url)
        .json(body)
        .send()
        .await
        .map_err(|e| ApiClientError::Network(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        #[cfg(target_arch = "wasm32")]
        let body = with_api_timeout(response.text(), |e| ApiClientError::Network(e.to_string()))
            .await
            .unwrap_or_default();
        #[cfg(not(target_arch = "wasm32"))]
        let body = response.text().await.unwrap_or_default();
        return Err(ApiClientError::Http(status.as_u16(), body));
    }

    #[cfg(target_arch = "wasm32")]
    {
        with_api_timeout(response.json::<Res>(), |e| {
            ApiClientError::Parse(e.to_string())
        })
        .await
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        response
            .json::<Res>()
            .await
            .map_err(|e| ApiClientError::Parse(e.to_string()))
    }
}

#[cfg(test)]
#[path = "client/tests.rs"]
mod tests;
