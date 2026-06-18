//! Errors the application (use-case) layer raises. Infrastructure maps
//! `sqlx::Error::RowNotFound -> NotFound` and everything else -> `Internal`.
//! The web layer turns these into HTTP status + JSON envelope.

use thiserror::Error;

use crate::domain::error::DomainError;

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("validation: {0}")]
    Validation(String),
    #[error("internal: {0}")]
    Internal(String),
}

impl ApplicationError {
    pub fn not_found(what: impl Into<String>) -> Self {
        ApplicationError::NotFound(what.into())
    }

    pub fn internal(detail: impl std::fmt::Display) -> Self {
        ApplicationError::Internal(detail.to_string())
    }
}

/// Domain invariant violations surface as validation failures to the caller.
impl From<DomainError> for ApplicationError {
    fn from(e: DomainError) -> Self {
        ApplicationError::Validation(e.to_string())
    }
}

pub type AppResult<T> = Result<T, ApplicationError>;
