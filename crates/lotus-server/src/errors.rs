// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Error handling and HTTP response mapping for LOTUS API endpoints.

use axum::{Json, http::StatusCode, response::IntoResponse, response::Response};
use serde::Serialize;
use utoipa::ToSchema;

/// JSON error envelope returned for failed API requests.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}

/// Typed HTTP API error mapped to an Axum JSON response.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
    /// Seconds to wait before trying again, sent as `Retry-After`.
    pub retry_after: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SharedApiError {
    pub status: StatusCode,
    pub message: String,
    /// Carried through from leader to coalesced followers so a shed search's
    /// `Retry-After` reaches every caller.
    pub retry_after: Option<u64>,
}

impl ApiError {
    #[must_use]
    pub fn upstream_overloaded(message: impl Into<String>, retry_after: u64) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
            retry_after: Some(retry_after),
        }
    }

    #[must_use]
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
            retry_after: None,
        }
    }

    #[must_use]
    pub fn upstream(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: message.into(),
            retry_after: None,
        }
    }

    #[must_use]
    pub fn overloaded(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
            retry_after: None,
        }
    }

    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
            retry_after: None,
        }
    }

    #[must_use]
    pub fn timed_out(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::GATEWAY_TIMEOUT,
            message: message.into(),
            retry_after: None,
        }
    }

    #[must_use]
    pub fn from_fetch(context: &str, err: lotus_search::FetchError) -> Self {
        match err {
            err @ lotus_search::FetchError::TimedOut { .. } => {
                Self::timed_out(format!("{context}: {err}"))
            }
            other => Self::upstream(format!("{context}: {other}")),
        }
    }
}

impl SharedApiError {
    #[must_use]
    pub fn timed_out(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::GATEWAY_TIMEOUT,
            message: message.into(),
            retry_after: None,
        }
    }
}

impl From<ApiError> for SharedApiError {
    fn from(value: ApiError) -> Self {
        Self {
            status: value.status,
            message: value.message,
            retry_after: value.retry_after,
        }
    }
}

impl From<SharedApiError> for ApiError {
    fn from(value: SharedApiError) -> Self {
        Self {
            status: value.status,
            message: value.message,
            retry_after: value.retry_after,
        }
    }
}

impl From<axum::http::Error> for ApiError {
    fn from(err: axum::http::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("response builder: {err}"),
            retry_after: None,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            error: self.message,
        });
        let mut response = (self.status, body).into_response();
        if let Some(seconds) = self.retry_after {
            response.headers_mut().insert("retry-after", seconds.into());
        }
        response
    }
}
