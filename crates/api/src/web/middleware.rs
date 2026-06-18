//! Cross-cutting HTTP layers. The CORS policy is built from the configured
//! allowed origins; request-id, tracing, and the body-size limit are wired in
//! `router::build_router`.

use axum::http::{HeaderValue, Method, header};
use tower_http::cors::CorsLayer;

use crate::config::Settings;

/// Build a CORS layer from `CORS_ALLOWED_ORIGINS`. An unparseable origin is
/// skipped rather than aborting startup.
pub fn cors_layer(settings: &Settings) -> CorsLayer {
    let origins: Vec<HeaderValue> = settings
        .cors_allowed_origins
        .iter()
        .filter_map(|o| o.parse::<HeaderValue>().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
}
