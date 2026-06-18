//! Router assembly for the public resolver service.

use axum::Router;
use axum::routing::get;
use tower_http::cors::{Any, CorsLayer};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

use crate::handlers;
use crate::state::ResolverState;

pub fn build_router(state: ResolverState) -> Router {
    // This is a public, read-only API hit from QR scanners on arbitrary origins,
    // so allow any origin for GETs. No credentials/cookies are involved (auth is
    // a bearer token the caller supplies explicitly).
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([axum::http::Method::GET]);

    Router::new()
        .route("/health", get(handlers::health))
        .route("/health/ready", get(handlers::ready))
        .route("/r/{code}", get(handlers::summary))
        .route("/r/{code}/photo", get(handlers::photo))
        .route("/r/{code}/full", get(handlers::full))
        .layer(cors)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(TraceLayer::new_for_http())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .with_state(state)
}
