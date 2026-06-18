//! Auth endpoints: register, login, refresh (rotating), logout, me.

use axum::{Json, extract::State, http::StatusCode};

use crate::error::ApiResult;
use crate::state::AppState;
use crate::web::dto::{
    LoginRequest, LogoutRequest, MeResponse, RefreshRequest, RegisterRequest, TokenResponse,
};
use crate::web::extractors::{AuthUser, ValidatedJson};

pub async fn register(
    State(st): State<AppState>,
    ValidatedJson(req): ValidatedJson<RegisterRequest>,
) -> ApiResult<(StatusCode, Json<TokenResponse>)> {
    let (_user, _org, tokens) = st
        .auth
        .register(
            req.organization_name,
            req.email,
            req.password,
            req.full_name,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(tokens.into())))
}

pub async fn login(
    State(st): State<AppState>,
    ValidatedJson(req): ValidatedJson<LoginRequest>,
) -> ApiResult<Json<TokenResponse>> {
    let tokens = st.auth.login(req.email, req.password).await?;
    Ok(Json(tokens.into()))
}

pub async fn refresh(
    State(st): State<AppState>,
    ValidatedJson(req): ValidatedJson<RefreshRequest>,
) -> ApiResult<Json<TokenResponse>> {
    let tokens = st.auth.refresh(req.refresh_token).await?;
    Ok(Json(tokens.into()))
}

pub async fn logout(
    State(st): State<AppState>,
    ValidatedJson(req): ValidatedJson<LogoutRequest>,
) -> ApiResult<StatusCode> {
    st.auth.logout(req.refresh_token).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn me(State(st): State<AppState>, user: AuthUser) -> ApiResult<Json<MeResponse>> {
    let (u, o) = st.auth.me(user.organization_id, user.user_id).await?;
    Ok(Json(MeResponse {
        user: u.into(),
        organization: o.into(),
    }))
}
