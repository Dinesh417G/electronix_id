//! Shared state for the resolver service.
//!
//! Composed from the api crate's MySQL adapters — the resolver owns no
//! persistence code of its own. It needs read access to machines (lookup by
//! public code), documents/versions (the passport contents), organizations
//! (the org name on the public summary), file storage (streaming the primary
//! photo), and the token verifier (gating the full view).

use std::sync::Arc;

use sqlx::MySqlPool;

use electronix_id_api::application::ports::document_repo::DocumentRepository;
use electronix_id_api::application::ports::file_storage::FileStorage;
use electronix_id_api::application::ports::machine_repo::MachineRepository;
use electronix_id_api::application::ports::organization_repo::OrganizationRepository;
use electronix_id_api::application::ports::token_service::TokenService;
use electronix_id_api::config::Settings;
use electronix_id_api::infrastructure::persistence::mysql_document_repo::MySqlDocumentRepo;
use electronix_id_api::infrastructure::persistence::mysql_machine_repo::MySqlMachineRepo;
use electronix_id_api::infrastructure::persistence::mysql_organization_repo::MySqlOrganizationRepo;
use electronix_id_api::infrastructure::security::jwt_token_service::JwtTokenService;
use electronix_id_api::infrastructure::storage::local_file_storage::LocalFileStorage;

#[derive(Clone)]
pub struct ResolverState {
    /// Held only for the readiness probe (`SELECT 1`).
    pub pool: MySqlPool,
    pub machines: Arc<dyn MachineRepository>,
    pub documents: Arc<dyn DocumentRepository>,
    pub orgs: Arc<dyn OrganizationRepository>,
    pub storage: Arc<dyn FileStorage>,
    pub tokens: Arc<dyn TokenService>,
}

impl ResolverState {
    pub fn new(pool: MySqlPool, settings: &Settings) -> Self {
        let machines = Arc::new(MySqlMachineRepo::new(pool.clone()));
        let documents = Arc::new(MySqlDocumentRepo::new(pool.clone()));
        let orgs = Arc::new(MySqlOrganizationRepo::new(pool.clone()));
        let storage = Arc::new(LocalFileStorage::new(settings.storage_root.clone()));
        // The resolver only ever verifies tokens; the access TTL is irrelevant
        // here but the constructor requires one.
        let tokens = Arc::new(JwtTokenService::new(
            settings.jwt_secret.as_bytes(),
            settings.access_token_ttl.as_secs() as i64,
        ));

        Self {
            pool,
            machines,
            documents,
            orgs,
            storage,
            tokens,
        }
    }
}
