//! Shared application state handed to every handler.
//!
//! Holds the pool (for health checks) + settings + the six application
//! services. [`AppState::new`] wires the MySQL repositories and infra adapters
//! into the services — the single composition root for the running binary and
//! the `#[sqlx::test]` handler tests.

use std::sync::Arc;

use sqlx::MySqlPool;

use crate::application::auth_service::AuthService;
use crate::application::document_service::DocumentService;
use crate::application::machine_service::MachineService;
use crate::application::organization_service::OrganizationService;
use crate::application::pricing_service::PricingService;
use crate::application::user_service::UserService;
use crate::config::Settings;
use crate::infrastructure::persistence::mysql_document_repo::MySqlDocumentRepo;
use crate::infrastructure::persistence::mysql_machine_repo::MySqlMachineRepo;
use crate::infrastructure::persistence::mysql_organization_repo::MySqlOrganizationRepo;
use crate::infrastructure::persistence::mysql_plan_repo::MySqlPlanRepo;
use crate::infrastructure::persistence::mysql_refresh_token_repo::MySqlRefreshTokenRepo;
use crate::infrastructure::persistence::mysql_subscription_repo::MySqlSubscriptionRepo;
use crate::infrastructure::persistence::mysql_user_repo::MySqlUserRepo;
use crate::infrastructure::security::argon2_hasher::Argon2Hasher;
use crate::infrastructure::security::jwt_token_service::JwtTokenService;
use crate::infrastructure::storage::local_file_storage::LocalFileStorage;

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub settings: Arc<Settings>,
    pub auth: AuthService,
    pub users: UserService,
    pub organization: OrganizationService,
    pub machines: MachineService,
    pub documents: DocumentService,
    pub pricing: PricingService,
}

impl AppState {
    /// Compose the MySQL-backed services from a pool + settings.
    pub fn new(pool: MySqlPool, settings: Arc<Settings>) -> Self {
        let orgs = Arc::new(MySqlOrganizationRepo::new(pool.clone()));
        let users_repo = Arc::new(MySqlUserRepo::new(pool.clone()));
        let machines_repo = Arc::new(MySqlMachineRepo::new(pool.clone()));
        let plans_repo = Arc::new(MySqlPlanRepo::new(pool.clone()));
        let subs_repo = Arc::new(MySqlSubscriptionRepo::new(pool.clone()));
        let refresh_repo = Arc::new(MySqlRefreshTokenRepo::new(pool.clone()));
        let docs_repo = Arc::new(MySqlDocumentRepo::new(pool.clone()));

        let hasher = Arc::new(Argon2Hasher::default());
        let tokens = Arc::new(JwtTokenService::new(
            settings.jwt_secret.as_bytes(),
            settings.access_token_ttl.as_secs() as i64,
        ));
        let storage = Arc::new(LocalFileStorage::new(settings.storage_root.clone()));

        let auth = AuthService::new(
            users_repo.clone(),
            orgs.clone(),
            subs_repo.clone(),
            refresh_repo.clone(),
            hasher.clone(),
            tokens.clone(),
            settings.access_token_ttl.as_secs() as i64,
            settings.refresh_token_ttl.as_secs() as i64,
        );
        let users = UserService::new(users_repo.clone(), hasher.clone());
        let organization = OrganizationService::new(orgs.clone());
        let machines = MachineService::new(machines_repo.clone(), plans_repo.clone());
        let documents = DocumentService::new(docs_repo.clone(), machines_repo.clone(), storage);
        let pricing = PricingService::new(machines_repo, plans_repo, subs_repo);

        Self {
            pool,
            settings,
            auth,
            users,
            organization,
            machines,
            documents,
            pricing,
        }
    }
}
