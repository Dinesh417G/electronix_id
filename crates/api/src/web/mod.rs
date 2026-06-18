//! Web layer — axum router, handlers, extractors, DTOs. Depends on
//! `application` + `domain`. No business logic lives here.

pub mod dto;
pub mod extractors;
pub mod handlers;
pub mod middleware;
pub mod pagination;
pub mod router;

use std::str::FromStr;

use crate::error::AppError;

/// Parse a path-segment id into a domain newtype. A malformed id cannot name an
/// existing resource, so it surfaces as `NotFound` rather than a 400.
pub(crate) fn parse_id<T: FromStr>(raw: &str, what: &str) -> Result<T, AppError> {
    T::from_str(raw).map_err(|_| AppError::NotFound(format!("{what} not found")))
}
