//! Machine CRUD + tier assignment. Writes require engineer+; delete/tier require
//! admin+ (enforced in `MachineService`).

use std::str::FromStr;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};

use crate::application::machine_service::{MachinePatch, NewMachine};
use crate::domain::ids::MachineId;
use crate::domain::value_objects::{MachineStatus, Tier};
use crate::error::{ApiResult, AppError};
use crate::state::AppState;
use crate::web::dto::{CreateMachineRequest, MachineResponse, TierRequest, UpdateMachineRequest};
use crate::web::extractors::{AuthUser, ValidatedJson};
use crate::web::pagination::{Page, PageParams};
use crate::web::parse_id;

fn parse_status(raw: &str) -> ApiResult<MachineStatus> {
    MachineStatus::from_str(raw).map_err(AppError::from)
}

pub async fn list(
    State(st): State<AppState>,
    user: AuthUser,
    Query(params): Query<PageParams>,
) -> ApiResult<Json<Page<MachineResponse>>> {
    let (limit, offset, page, per_page) = params.resolve();
    let (machines, total) = st
        .machines
        .list(user.organization_id, limit, offset)
        .await?;
    let data = machines.into_iter().map(MachineResponse::from).collect();
    Ok(Json(Page::new(data, page, per_page, total)))
}

pub async fn create(
    State(st): State<AppState>,
    user: AuthUser,
    ValidatedJson(req): ValidatedJson<CreateMachineRequest>,
) -> ApiResult<(StatusCode, Json<MachineResponse>)> {
    let status = match req.status {
        Some(s) => Some(parse_status(&s)?),
        None => None,
    };
    let input = NewMachine {
        name: req.name,
        make: req.make,
        model: req.model,
        serial_number: req.serial_number,
        asset_tag: req.asset_tag,
        location: req.location,
        year_installed: req.year_installed,
        status,
    };
    let machine = st
        .machines
        .create(user.organization_id, user.role, user.user_id, input)
        .await?;
    Ok((StatusCode::CREATED, Json(machine.into())))
}

pub async fn get(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<MachineResponse>> {
    let id: MachineId = parse_id(&id, "machine")?;
    let machine = st.machines.get(user.organization_id, id).await?;
    Ok(Json(machine.into()))
}

pub async fn update(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    ValidatedJson(req): ValidatedJson<UpdateMachineRequest>,
) -> ApiResult<Json<MachineResponse>> {
    let id: MachineId = parse_id(&id, "machine")?;
    let status = match req.status {
        Some(s) => Some(parse_status(&s)?),
        None => None,
    };
    // A present field updates; nullable fields are set to the given value.
    let patch = MachinePatch {
        name: req.name,
        make: req.make.map(Some),
        model: req.model.map(Some),
        serial_number: req.serial_number.map(Some),
        asset_tag: req.asset_tag.map(Some),
        location: req.location.map(Some),
        year_installed: req.year_installed.map(Some),
        status,
    };
    let machine = st
        .machines
        .update(user.organization_id, user.role, id, patch)
        .await?;
    Ok(Json(machine.into()))
}

pub async fn delete(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let id: MachineId = parse_id(&id, "machine")?;
    st.machines
        .delete(user.organization_id, user.role, id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_tier(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    ValidatedJson(req): ValidatedJson<TierRequest>,
) -> ApiResult<Json<MachineResponse>> {
    let id: MachineId = parse_id(&id, "machine")?;
    let tier = Tier::from_str(&req.plan_code).map_err(AppError::from)?;
    let machine = st
        .machines
        .set_tier(user.organization_id, user.role, id, tier)
        .await?;
    Ok(Json(machine.into()))
}
