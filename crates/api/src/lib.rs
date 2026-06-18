//! ElectronIx ID backend library. The `api` binary is a thin wrapper over
//! [`run`]; tests link against this crate to exercise services and handlers.

pub mod application;
pub mod config;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod state;
pub mod web;

use std::sync::Arc;

use anyhow::Context;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{config::Settings, infrastructure::db, state::AppState, web::router::build_router};

/// Initialise the global tracing subscriber from `RUST_LOG`. Safe to skip in
/// tests (ignored if already set).
pub fn init_tracing() {
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

/// Load config, connect + migrate, build the router, and serve until shutdown.
pub async fn run() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let settings = Arc::new(Settings::from_env().context("failed to load configuration")?);

    let pool = db::build_pool(&settings.database_url)
        .await
        .context("failed to connect to database")?;

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;

    let state = AppState::new(pool, settings.clone());
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(&settings.bind_addr)
        .await
        .with_context(|| format!("failed to bind {}", settings.bind_addr))?;
    tracing::info!("listening on {}", settings.bind_addr);

    axum::serve(listener, app).await.context("server error")?;

    Ok(())
}
