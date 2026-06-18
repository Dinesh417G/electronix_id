//! Pricing catalog. (Subscription + estimate live under `organization`.)

use axum::{Json, extract::State};

use crate::error::ApiResult;
use crate::state::AppState;
use crate::web::dto::PlanResponse;
use crate::web::extractors::AuthUser;
use crate::web::pagination::Data;

/// The full plan catalog, including roadmap (inactive) plans.
pub async fn list_plans(
    State(st): State<AppState>,
    _user: AuthUser,
) -> ApiResult<Json<Data<PlanResponse>>> {
    let plans = st.pricing.plans().await?;
    let data = plans.into_iter().map(PlanResponse::from).collect();
    Ok(Json(Data::new(data)))
}
