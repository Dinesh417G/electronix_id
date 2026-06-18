//! Blob storage port. `LocalFileStorage` (tokio::fs) implements it now; an R2
//! adapter drops in behind the same trait later. Keys are caller-decided
//! (`{org}/{machine}/{document}/{version}/{filename}`).

use async_trait::async_trait;

use crate::application::error::AppResult;

#[async_trait]
pub trait FileStorage: Send + Sync {
    async fn put(&self, key: &str, bytes: &[u8]) -> AppResult<()>;
    async fn get(&self, key: &str) -> AppResult<Vec<u8>>;
    async fn delete(&self, key: &str) -> AppResult<()>;
}
