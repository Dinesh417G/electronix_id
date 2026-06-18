//! Organization read/update + the billing read endpoints.

use axum::{Json, extract::State};

use crate::error::ApiResult;
use crate::state::AppState;
use crate::web::dto::{
    EstimateResponse, OrganizationResponse, SubscriptionResponse, UpdateOrganizationRequest,
};
use crate::web::extractors::{AuthUser, ValidatedJson};

pub async fn get(
    State(st): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<OrganizationResponse>> {
    let org = st.organization.get(user.organization_id).await?;
    Ok(Json(org.into()))
}

pub async fn update(
    State(st): State<AppState>,
    user: AuthUser,
    ValidatedJson(req): ValidatedJson<UpdateOrganizationRequest>,
) -> ApiResult<Json<OrganizationResponse>> {
    let org = st
        .organization
        .update(user.organization_id, user.role, req.name)
        .await?;
    Ok(Json(org.into()))
}

pub async fn subscription(
    State(st): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<SubscriptionResponse>> {
    let sub = st.pricing.subscription(user.organization_id).await?;
    Ok(Json(sub.into()))
}

pub async fn estimate(
    State(st): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<EstimateResponse>> {
    let est = st.pricing.estimate(user.organization_id).await?;
    Ok(Json(est.into()))
}
