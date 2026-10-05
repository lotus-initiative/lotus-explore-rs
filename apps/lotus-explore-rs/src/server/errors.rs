// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: Contributors to the lotus-explore-rs project
//! Error handling and HTTP response mapping for LOTUS API endpoints.

use axum::{Json, http::StatusCode, response::IntoResponse, response::Response};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub(crate) error: String,
}

#[derive(Debug)]
pub struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
    /// Seconds to wait before trying again, sent as `Retry-After`.
    ///
    /// Present on the overload path and absent everywhere else. It is the whole
    /// difference between "this server is busy" and "here is when to come back":
    /// a client told to retry immediately becomes the reason the server is busy.
    pub(crate) retry_after: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SharedApiError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
    /// Carried through, not dropped.
    ///
    /// A *leader* hands this to the followers coalesced behind it, so a shed
    /// search's `Retry-After` reaches the caller. Zeroing it because a follower
    /// has no queue to join is beside the point: the header is what stops the
    /// follower coming straight back.
    pub(crate) retry_after: Option<u64>,
}

impl ApiError {
    /// The server is at its upstream budget; the answer carries a `Retry-After`.
    /// Shedding at the door is the polite half of a concurrency limit — the
    /// alternative is accepting the request, starting a query the shared public
    /// endpoint will cancel, then telling the caller to wait.
    #[must_use]
    pub(crate) fn upstream_overloaded(message: impl Into<String>, retry_after: u64) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
            retry_after: Some(retry_after),
        }
    }

    #[must_use]
    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
            retry_after: None,
        }
    }

    #[must_use]
    pub(crate) fn upstream(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: message.into(),
            retry_after: None,
        }
    }

    #[must_use]
    pub(crate) fn overloaded(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
            retry_after: None,
        }
    }

    #[must_use]
    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
            retry_after: None,
        }
    }
}

impl SharedApiError {
    /// A local timeout, as opposed to an endpoint one.
    #[must_use]
    pub(crate) fn timed_out(message: impl Into<String>) -> Self {
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

// `http::response::Builder::body` returns `Result<_, http::Error>`. The only
// ways it can fail are invalid header names/values or a body that fails to
// encode — both impossible with the static literals used in handlers.rs, but
// propagating the typed `ApiError` (500) is strictly safer than `.expect`.
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
            // `response.extensions_mut()` rather than rebuilding through a
            // `Builder`: the status and body are already a `Response`, and
            // rebuilding them here would be a second place that has to be right.
            response.headers_mut().insert("retry-after", seconds.into());
        }
        response
    }
}
