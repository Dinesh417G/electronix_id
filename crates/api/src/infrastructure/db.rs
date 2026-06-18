//! MySQL connection pool builder.

use std::time::Duration;

use sqlx::mysql::{MySqlPool, MySqlPoolOptions};

pub async fn build_pool(database_url: &str) -> Result<MySqlPool, sqlx::Error> {
    MySqlPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .connect(database_url)
        .await
}
