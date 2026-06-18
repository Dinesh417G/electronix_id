//! MySQL persistence adapters — the only place that knows `sqlx`/MySQL exists.
//! Each repo maps rows ↔ domain types and runs SQL; no business rules here.
//!
//! Conventions shared by every repo:
//! - ids are `CHAR(36)`, bound as `.to_string()`, read back as `String` and
//!   parsed via [`parse_uuid`];
//! - timestamps are `DATETIME(6)` storing UTC: bound as `.naive_utc()`, read as
//!   `NaiveDateTime` and lifted to `DateTime<Utc>` via [`to_utc`];
//! - JSON columns are read with `CAST(col AS CHAR)` so the domain keeps raw text;
//! - errors map `RowNotFound → NotFound`, unique violations → `Conflict`, the
//!   rest → `Internal` (logged).

pub mod mysql_document_repo;
pub mod mysql_machine_repo;
pub mod mysql_organization_repo;
pub mod mysql_plan_repo;
pub mod mysql_refresh_token_repo;
pub mod mysql_subscription_repo;
pub mod mysql_user_repo;

use chrono::{DateTime, NaiveDateTime, Utc};
use uuid::Uuid;

use crate::application::error::ApplicationError;

/// Lift a naive UTC `DATETIME(6)` reading into a timezone-aware value.
pub(crate) fn to_utc(n: NaiveDateTime) -> DateTime<Utc> {
    DateTime::from_naive_utc_and_offset(n, Utc)
}

/// Parse a `CHAR(36)` id column into one of the domain id newtypes.
pub(crate) fn parse_uuid<T: From<Uuid>>(s: &str) -> Result<T, ApplicationError> {
    Uuid::parse_str(s)
        .map(T::from)
        .map_err(ApplicationError::internal)
}

/// Map a `sqlx::Error` to an `ApplicationError`. `what` names the entity for a
/// helpful `NotFound`. Unexpected failures are logged and hidden as `Internal`.
pub(crate) fn map_sqlx(e: sqlx::Error, what: &str) -> ApplicationError {
    match e {
        sqlx::Error::RowNotFound => ApplicationError::NotFound(what.to_string()),
        sqlx::Error::Database(ref db) if db.is_unique_violation() => {
            ApplicationError::Conflict(format!("{what} already exists"))
        }
        other => {
            tracing::error!(error = %other, entity = what, "database error");
            ApplicationError::internal(other)
        }
    }
}
