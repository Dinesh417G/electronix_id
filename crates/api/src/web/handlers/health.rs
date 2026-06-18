//! Liveness + readiness probes.
//! `/health` answers if the process is up; `/health/ready` also pings the DB.

use axum::{Json, extract::State, http::StatusCode};
use serde_json::{Value, json};

use crate::state::AppState;

pub async fn liveness() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn readiness(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    match sqlx::query("SELECT 1").execute(&state.pool).await {
        Ok(_) => Ok(Json(json!({ "status": "ready" }))),
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}
