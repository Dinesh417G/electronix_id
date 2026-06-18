//! HTTP-facing error type. Maps every internal error to a status + JSON envelope:
//! `{ "error": { "code": "...", "message": "..." } }`.
//!
//! Conversions from `ApplicationError` / `DomainError` are added as those layers
//! land. Internal errors are logged here and never leaked to the client.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::application::error::ApplicationError;
use crate::domain::error::DomainError;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    Unauthorized(String),
    Forbidden(String),
    Conflict(String),
    Validation(String),
    Internal(String),
}

/// Map the application layer's errors onto HTTP-facing ones. This is the single
/// translation point — handlers return `AppError` and `?` their service calls.
impl From<ApplicationError> for AppError {
    fn from(e: ApplicationError) -> Self {
        match e {
            ApplicationError::NotFound(m) => AppError::NotFound(m),
            ApplicationError::Unauthorized(m) => AppError::Unauthorized(m),
            ApplicationError::Forbidden(m) => AppError::Forbidden(m),
            ApplicationError::Conflict(m) => AppError::Conflict(m),
            ApplicationError::Validation(m) => AppError::Validation(m),
            ApplicationError::Internal(m) => AppError::Internal(m),
        }
    }
}

/// Domain invariant violations surface as 422s to the client.
impl From<DomainError> for AppError {
    fn from(e: DomainError) -> Self {
        AppError::Validation(e.to_string())
    }
}

pub type ApiResult<T> = Result<T, AppError>;

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, "not_found", m),
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, "unauthorized", m),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, "forbidden", m),
            AppError::Conflict(m) => (StatusCode::CONFLICT, "conflict", m),
            AppError::Validation(m) => (StatusCode::UNPROCESSABLE_ENTITY, "validation", m),
            AppError::Internal(detail) => {
                tracing::error!(error = %detail, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "internal server error".to_string(),
                )
            }
        };

        (
            status,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: code.to_string(),
                    message,
                },
            }),
        )
            .into_response()
    }
}
