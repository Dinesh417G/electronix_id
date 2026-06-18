//! User management endpoints. Reads are org-scoped; writes require admin/owner
//! (enforced in `UserService`).

use std::str::FromStr;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};

use crate::application::user_service::UserPatch;
use crate::domain::ids::UserId;
use crate::domain::value_objects::Role;
use crate::error::{ApiResult, AppError};
use crate::state::AppState;
use crate::web::dto::{CreateUserRequest, UpdateUserRequest, UserResponse};
use crate::web::extractors::{AuthUser, ValidatedJson};
use crate::web::pagination::{Page, PageParams};
use crate::web::parse_id;

fn parse_role(raw: &str) -> ApiResult<Role> {
    Role::from_str(raw).map_err(AppError::from)
}

pub async fn list(
    State(st): State<AppState>,
    user: AuthUser,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<Page<UserResponse>>> {
    let (limit, offset, page, per_page) = params.resolve();
    let (users, total) = st.users.list(user.organization_id, limit, offset).await?;
    let data = users.into_iter().map(UserResponse::from).collect();
    Ok(Json(Page::new(data, page, per_page, total)))
}

pub async fn create(
    State(st): State<AppState>,
    user: AuthUser,
    ValidatedJson(req): ValidatedJson<CreateUserRequest>,
) -> ApiResult<(StatusCode, Json<UserResponse>)> {
    let role = parse_role(&req.role)?;
    let created = st
        .users
        .create(
            user.organization_id,
            user.role,
            req.email,
            req.password,
            req.full_name,
            role,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(created.into())))
}

pub async fn get(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<UserResponse>> {
    let id: UserId = parse_id(&id, "user")?;
    let found = st.users.get(user.organization_id, id).await?;
    Ok(Json(found.into()))
}

pub async fn update(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    ValidatedJson(req): ValidatedJson<UpdateUserRequest>,
) -> ApiResult<Json<UserResponse>> {
    let id: UserId = parse_id(&id, "user")?;
    let role = match req.role {
        Some(r) => Some(parse_role(&r)?),
        None => None,
    };
    let patch = UserPatch {
        full_name: req.full_name,
        role,
        is_active: req.is_active,
        password: req.password,
    };
    let updated = st
        .users
        .update(user.organization_id, user.role, id, patch)
        .await?;
    Ok(Json(updated.into()))
}

pub async fn delete(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let id: UserId = parse_id(&id, "user")?;
    st.users.delete(user.organization_id, user.role, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
