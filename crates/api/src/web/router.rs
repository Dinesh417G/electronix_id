//! Router assembly: `/health` at the root, everything else under `/api/v1`,
//! wrapped with request-id, tracing, CORS, and a body-size limit.

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, patch, post};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

use crate::state::AppState;
use crate::web::handlers::{auth, documents, health, machines, organization, pricing, users};
use crate::web::middleware::cors_layer;

pub fn build_router(state: AppState) -> Router {
    let max_body = state.settings.max_upload_bytes;
    let cors = cors_layer(&state.settings);

    let api = Router::new()
        // auth
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        // users
        .route("/users", get(users::list).post(users::create))
        .route(
            "/users/{id}",
            get(users::get).patch(users::update).delete(users::delete),
        )
        // organization + billing
        .route(
            "/organization",
            get(organization::get).patch(organization::update),
        )
        .route(
            "/organization/subscription",
            get(organization::subscription),
        )
        .route(
            "/organization/billing/estimate",
            get(organization::estimate),
        )
        // machines
        .route("/machines", get(machines::list).post(machines::create))
        .route(
            "/machines/{id}",
            get(machines::get)
                .patch(machines::update)
                .delete(machines::delete),
        )
        .route("/machines/{id}/tier", patch(machines::set_tier))
        .route("/machines/{id}/tag/rotate", post(machines::rotate_tag))
        // documents on a machine
        .route(
            "/machines/{machine_id}/documents",
            get(documents::list_for_machine).post(documents::create_slot),
        )
        // documents + versions
        .route(
            "/documents/{document_id}",
            get(documents::get)
                .patch(documents::update)
                .delete(documents::delete),
        )
        .route(
            "/documents/{document_id}/versions",
            get(documents::list_versions).post(documents::add_version),
        )
        .route(
            "/documents/{document_id}/versions/{version_no}",
            get(documents::get_version),
        )
        .route(
            "/documents/{document_id}/versions/{version_no}/download",
            get(documents::download),
        )
        .route(
            "/documents/{document_id}/versions/{version_no}/restore",
            post(documents::restore),
        )
        // pricing catalog
        .route("/plans", get(pricing::list_plans));

    Router::new()
        .route("/health", get(health::liveness))
        .route("/health/ready", get(health::readiness))
        .nest("/api/v1", api)
        // Uploads can be large; raise axum's default 2 MB limit and cap at config.
        .layer(DefaultBodyLimit::max(max_body))
        .layer(RequestBodyLimitLayer::new(max_body))
        .layer(cors)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(TraceLayer::new_for_http())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .with_state(state)
}
