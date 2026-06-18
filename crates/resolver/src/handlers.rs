//! Resolver endpoints.
//!
//! - `GET /r/{code}` — public passport summary (no auth).
//! - `GET /r/{code}/photo` — public primary-photo stream (no auth).
//! - `GET /r/{code}/full` — full passport; requires a token whose org owns the
//!   machine. A cross-org token gets 404 (no existence leak).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use electronix_id_api::domain::machine::Machine;
use electronix_id_api::error::{ApiResult, AppError};

use crate::auth::ScanViewer;
use crate::dto::{PassportDocument, PassportFull, PassportSummary};
use crate::state::ResolverState;

pub async fn health() -> &'static str {
    "ok"
}

pub async fn ready(State(st): State<ResolverState>) -> ApiResult<StatusCode> {
    sqlx::query("SELECT 1")
        .execute(&st.pool)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(StatusCode::OK)
}

/// Resolve a public code to a machine, or 404. The code itself is the
/// capability for the public summary; ownership is checked separately for the
/// gated views.
async fn lookup(st: &ResolverState, code: &str) -> ApiResult<Machine> {
    st.machines
        .find_by_public_code(code)
        .await?
        .ok_or_else(|| AppError::NotFound("machine".into()))
}

pub async fn summary(
    State(st): State<ResolverState>,
    Path(code): Path<String>,
) -> ApiResult<Json<PassportSummary>> {
    let machine = lookup(&st, &code).await?;
    let org = st.orgs.find_by_id(machine.organization_id).await?;
    Ok(Json(PassportSummary::build(&machine, &org)))
}

pub async fn full(
    viewer: ScanViewer,
    State(st): State<ResolverState>,
    Path(code): Path<String>,
) -> ApiResult<Json<PassportFull>> {
    let machine = lookup(&st, &code).await?;

    // Org gate: a valid token from another org must not see this machine's
    // documents — and must not even learn it exists. Answer 404, same as the api.
    if viewer.0.org != machine.organization_id {
        return Err(AppError::NotFound("machine".into()));
    }

    let org = st.orgs.find_by_id(machine.organization_id).await?;
    let docs = st
        .documents
        .list_by_machine(machine.organization_id, machine.id, None)
        .await?;

    let mut documents = Vec::with_capacity(docs.len());
    for doc in docs {
        let current = st
            .documents
            .current_version(machine.organization_id, doc.id)
            .await?;
        documents.push(PassportDocument::build(doc, current));
    }

    Ok(Json(PassportFull {
        summary: PassportSummary::build(&machine, &org),
        documents,
    }))
}

pub async fn photo(
    State(st): State<ResolverState>,
    Path(code): Path<String>,
) -> ApiResult<Response> {
    let machine = lookup(&st, &code).await?;
    let version_id = machine
        .primary_photo_version_id
        .ok_or_else(|| AppError::NotFound("photo".into()))?;

    // The version id comes from the machine's own pointer, so this unscoped
    // lookup is already bound to the right org.
    let version = st
        .documents
        .find_version_by_id(version_id)
        .await?
        .ok_or_else(|| AppError::NotFound("photo".into()))?;
    let key = version
        .storage_key
        .ok_or_else(|| AppError::NotFound("photo".into()))?;
    let bytes = st.storage.get(&key).await?;
    let mime = version
        .mime_type
        .unwrap_or_else(|| "application/octet-stream".to_string());

    Ok(([(header::CONTENT_TYPE, mime)], bytes).into_response())
}
