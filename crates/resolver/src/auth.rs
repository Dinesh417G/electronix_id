//! Optional auth for the resolver.
//!
//! The public summary needs no token. The *full* passport view is gated: it
//! requires a valid api access token and the caller's org must own the scanned
//! machine. `ScanViewer` verifies the bearer JWT and yields its claims; the
//! org-ownership check happens in the handler (so a cross-org token gets a 404,
//! not a leak that the machine exists).

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

use electronix_id_api::application::ports::token_service::AccessClaims;
use electronix_id_api::error::AppError;

use crate::state::ResolverState;

/// An authenticated scanner: the decoded access-token claims (user, org, role).
pub struct ScanViewer(pub AccessClaims);

impl FromRequestParts<ResolverState> for ScanViewer {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ResolverState,
    ) -> Result<Self, AppError> {
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

        let claims = state.tokens.verify_access(token)?;
        Ok(ScanViewer(claims))
    }
}
