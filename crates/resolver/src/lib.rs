//! ElectronIx ID **resolver** — the public QR/scan service.
//!
//! A separate binary from the tenant api (different trust boundary: this one is
//! reachable without a login). It maps a machine's opaque public tag code to a
//! passport view. It reuses the api crate's domain types, repository adapters,
//! security, and storage — it owns no persistence code and runs **no
//! migrations** (the api binary owns the schema).
//!
//! Scan model (decided with the user):
//! - `GET /r/{code}` and `/r/{code}/photo` are public (the unguessable code is
//!   the capability);
//! - `GET /r/{code}/full` requires a token whose org owns the machine.

pub mod auth;
pub mod config;
pub mod dto;
pub mod handlers;
pub mod router;
pub mod state;

use anyhow::Context;

use electronix_id_api::config::Settings;
use electronix_id_api::infrastructure::db;

/// Load config, connect to the (already-migrated) database, build the router,
/// and serve until shutdown.
pub async fn run() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    electronix_id_api::init_tracing();

    let settings = Settings::from_env().context("failed to load configuration")?;

    let pool = db::build_pool(&settings.database_url)
        .await
        .context("failed to connect to database")?;

    let state = state::ResolverState::new(pool, &settings);
    let app = router::build_router(state);

    let addr = config::resolver_bind_addr();
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;
    tracing::info!("resolver listening on {addr}");

    axum::serve(listener, app).await.context("server error")?;

    Ok(())
}
