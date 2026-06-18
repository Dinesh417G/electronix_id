//! Request extractors: `AuthUser` (validates the bearer JWT and loads the live
//! user) and `ValidatedJson<T>` (deserialize + `validator::Validate`, 422 on
//! failure). Protected handlers take `AuthUser` as an argument.

use axum::Json;
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use serde::de::DeserializeOwned;
use validator::Validate;

use crate::domain::ids::{OrgId, UserId};
use crate::domain::value_objects::Role;
use crate::error::AppError;
use crate::state::AppState;

/// The authenticated identity behind a request.
#[derive(Debug, Clone, Copy)]
pub struct AuthUser {
    pub user_id: UserId,
    pub organization_id: OrgId,
    pub role: Role,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("missing authorization header".into()))?;

        let token = header
            .strip_prefix("Bearer ")
            .or_else(|| header.strip_prefix("bearer "))
            .ok_or_else(|| AppError::Unauthorized("expected a Bearer token".into()))?
            .trim();

        let (user_id, organization_id, role) = state.auth.authenticate(token).await?;
        Ok(AuthUser {
            user_id,
            organization_id,
            role,
        })
    }
}

/// JSON body extractor that runs `validator::Validate` after deserialization.
pub struct ValidatedJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: DeserializeOwned + Validate,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, AppError> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|e| AppError::Validation(e.to_string()))?;
        value
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
        Ok(ValidatedJson(value))
    }
}
