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
}

#[derive(Debug, Clone)]
pub struct SharedApiError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

impl ApiError {
    #[must_use]
    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    #[must_use]
    pub(crate) fn upstream(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: message.into(),
        }
    }

    #[must_use]
    pub(crate) fn overloaded(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
        }
    }

    #[must_use]
    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl From<ApiError> for SharedApiError {
    fn from(value: ApiError) -> Self {
        Self {
            status: value.status,
            message: value.message,
        }
    }
}

impl From<SharedApiError> for ApiError {
    fn from(value: SharedApiError) -> Self {
        Self {
            status: value.status,
            message: value.message,
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
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            error: self.message,
        });
        (self.status, body).into_response()
    }
}
