//! Hand-written in-memory fakes implementing the application ports, plus a
//! `World` that wires them into the real services. No DB, no mockall — exactly
//! the §12 "services" test strategy.

#![allow(dead_code)]

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use electronix_id_api::application::auth_service::AuthService;
use electronix_id_api::application::document_service::DocumentService;
use electronix_id_api::application::error::{AppResult, ApplicationError};
use electronix_id_api::application::machine_service::MachineService;
use electronix_id_api::application::organization_service::OrganizationService;
use electronix_id_api::application::ports::document_repo::{DocumentRepository, NewVersionInput};
use electronix_id_api::application::ports::file_storage::FileStorage;
use electronix_id_api::application::ports::machine_repo::MachineRepository;
use electronix_id_api::application::ports::organization_repo::OrganizationRepository;
use electronix_id_api::application::ports::password_hasher::PasswordHasher;
use electronix_id_api::application::ports::plan_repo::PlanRepository;
use electronix_id_api::application::ports::refresh_token_repo::{
    RefreshTokenRecord, RefreshTokenRepository,
};
use electronix_id_api::application::ports::subscription_repo::SubscriptionRepository;
use electronix_id_api::application::ports::token_service::{AccessClaims, TokenService};
use electronix_id_api::application::ports::user_repo::UserRepository;
use electronix_id_api::application::pricing_service::PricingService;
use electronix_id_api::application::user_service::UserService;
use electronix_id_api::domain::document::{Document, DocumentVersion};
use electronix_id_api::domain::ids::{
    DocumentId, MachineId, OrgId, PlanId, RefreshTokenId, UserId, VersionId,
};
use electronix_id_api::domain::machine::Machine;
use electronix_id_api::domain::organization::Organization;
use electronix_id_api::domain::plan::{Plan, Subscription};
use electronix_id_api::domain::user::User;
use electronix_id_api::domain::value_objects::{
    Currency, DocumentCategory, Email, Money, Role, Tier,
};

fn not_found(what: &str) -> ApplicationError {
    ApplicationError::NotFound(what.to_string())
}

// ---------------------------------------------------------------------------
// Organizations
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct InMemoryOrgRepo {
    rows: Mutex<HashMap<OrgId, Organization>>,
}

#[async_trait]
impl OrganizationRepository for InMemoryOrgRepo {
    async fn create(&self, org: &Organization) -> AppResult<()> {
        self.rows.lock().unwrap().insert(org.id, org.clone());
        Ok(())
    }
    async fn find_by_id(&self, id: OrgId) -> AppResult<Organization> {
        self.rows
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| not_found("organization"))
    }
    async fn find_by_slug(&self, slug: &str) -> AppResult<Option<Organization>> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .values()
            .find(|o| o.slug == slug)
            .cloned())
    }
    async fn update(&self, org: &Organization) -> AppResult<()> {
        self.rows.lock().unwrap().insert(org.id, org.clone());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct InMemoryUserRepo {
    rows: Mutex<HashMap<UserId, User>>,
}

#[async_trait]
impl UserRepository for InMemoryUserRepo {
    async fn create(&self, user: &User) -> AppResult<()> {
        self.rows.lock().unwrap().insert(user.id, user.clone());
        Ok(())
    }
    async fn find_by_id(&self, org: OrgId, id: UserId) -> AppResult<User> {
        self.rows
            .lock()
            .unwrap()
            .get(&id)
            .filter(|u| u.organization_id == org)
            .cloned()
            .ok_or_else(|| not_found("user"))
    }
    async fn find_by_email(&self, email: &Email) -> AppResult<Option<User>> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .values()
            .find(|u| u.email.as_str() == email.as_str())
            .cloned())
    }
    async fn find_by_id_any(&self, id: UserId) -> AppResult<User> {
        self.rows
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| not_found("user"))
    }
    async fn list(&self, org: OrgId, limit: i64, offset: i64) -> AppResult<(Vec<User>, i64)> {
        let mut all: Vec<User> = self
            .rows
            .lock()
            .unwrap()
            .values()
            .filter(|u| u.organization_id == org)
            .cloned()
            .collect();
        all.sort_by_key(|u| u.created_at);
        let total = all.len() as i64;
        let page = all
            .into_iter()
            .skip(offset.max(0) as usize)
            .take(limit.max(0) as usize)
            .collect();
        Ok((page, total))
    }
    async fn update(&self, user: &User) -> AppResult<()> {
        self.rows.lock().unwrap().insert(user.id, user.clone());
        Ok(())
    }
    async fn delete(&self, org: OrgId, id: UserId) -> AppResult<()> {
        let mut rows = self.rows.lock().unwrap();
        match rows.get(&id) {
            Some(u) if u.organization_id == org => {
                rows.remove(&id);
                Ok(())
            }
            _ => Err(not_found("user")),
        }
    }
    async fn exists_by_email(&self, email: &Email) -> AppResult<bool> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .values()
            .any(|u| u.email.as_str() == email.as_str()))
    }
}

// ---------------------------------------------------------------------------
// Machines
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct InMemoryMachineRepo {
    rows: Mutex<HashMap<MachineId, Machine>>,
}

#[async_trait]
impl MachineRepository for InMemoryMachineRepo {
    async fn create(&self, machine: &Machine) -> AppResult<()> {
        self.rows
            .lock()
            .unwrap()
            .insert(machine.id, machine.clone());
        Ok(())
    }
    async fn find_by_id(&self, org: OrgId, id: MachineId) -> AppResult<Machine> {
        self.rows
            .lock()
            .unwrap()
            .get(&id)
            .filter(|m| m.organization_id == org)
            .cloned()
            .ok_or_else(|| not_found("machine"))
    }
    async fn find_by_public_code(&self, code: &str) -> AppResult<Option<Machine>> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .values()
            .find(|m| m.public_code.as_deref() == Some(code))
            .cloned())
    }
    async fn list(&self, org: OrgId, limit: i64, offset: i64) -> AppResult<(Vec<Machine>, i64)> {
        let mut all: Vec<Machine> = self
            .rows
            .lock()
            .unwrap()
            .values()
            .filter(|m| m.organization_id == org)
            .cloned()
            .collect();
        all.sort_by_key(|m| m.created_at);
        let total = all.len() as i64;
        let page = all
            .into_iter()
            .skip(offset.max(0) as usize)
            .take(limit.max(0) as usize)
            .collect();
        Ok((page, total))
    }
    async fn list_all(&self, org: OrgId) -> AppResult<Vec<Machine>> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .values()
            .filter(|m| m.organization_id == org)
            .cloned()
            .collect())
    }
    async fn update(&self, machine: &Machine) -> AppResult<()> {
        self.rows
            .lock()
            .unwrap()
            .insert(machine.id, machine.clone());
        Ok(())
    }
    async fn delete(&self, org: OrgId, id: MachineId) -> AppResult<()> {
        let mut rows = self.rows.lock().unwrap();
        match rows.get(&id) {
            Some(m) if m.organization_id == org => {
                rows.remove(&id);
                Ok(())
            }
            _ => Err(not_found("machine")),
        }
    }
}

// ---------------------------------------------------------------------------
// Plans (seeded catalog)
// ---------------------------------------------------------------------------

pub struct InMemoryPlanRepo {
    rows: Vec<Plan>,
}

impl InMemoryPlanRepo {
    pub fn seeded() -> Self {
        let inr = Currency::inr();
        let now = Utc::now();
        let mk =
            |code: Tier, name: &str, price: i64, onboarding: i64, features: &str, active: bool| {
                Plan {
                    id: PlanId::new(),
                    code,
                    name: name.to_string(),
                    price_per_machine_year: Money::new(price, inr.clone()),
                    onboarding_fee: Money::new(onboarding, inr.clone()),
                    features: Some(features.to_string()),
                    is_active: active,
                    created_at: now,
                }
            };
        Self {
            rows: vec![
                mk(
                    Tier::Basic,
                    "Passport Basic",
                    150_000,
                    60_000,
                    r#"{"static_passport":true,"live_data":false,"predict":false}"#,
                    true,
                ),
                mk(
                    Tier::Live,
                    "Passport Live",
                    360_000,
                    60_000,
                    r#"{"static_passport":true,"live_data":true,"predict":false}"#,
                    true,
                ),
                mk(
                    Tier::Predict,
                    "Passport Predict",
                    0,
                    60_000,
                    r#"{"static_passport":true,"live_data":true,"predict":true}"#,
                    false,
                ),
            ],
        }
    }

    pub fn id_of(&self, code: Tier) -> PlanId {
        self.rows.iter().find(|p| p.code == code).unwrap().id
    }
}

#[async_trait]
impl PlanRepository for InMemoryPlanRepo {
    async fn list(&self, active_only: bool) -> AppResult<Vec<Plan>> {
        Ok(self
            .rows
            .iter()
            .filter(|p| !active_only || p.is_active)
            .cloned()
            .collect())
    }
    async fn find_by_id(&self, id: PlanId) -> AppResult<Option<Plan>> {
        Ok(self.rows.iter().find(|p| p.id == id).cloned())
    }
    async fn find_by_code(&self, code: Tier) -> AppResult<Option<Plan>> {
        Ok(self.rows.iter().find(|p| p.code == code).cloned())
    }
}

// ---------------------------------------------------------------------------
// Subscriptions
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct InMemorySubscriptionRepo {
    rows: Mutex<HashMap<OrgId, Subscription>>,
}

#[async_trait]
impl SubscriptionRepository for InMemorySubscriptionRepo {
    async fn create(&self, sub: &Subscription) -> AppResult<()> {
        self.rows
            .lock()
            .unwrap()
            .insert(sub.organization_id, sub.clone());
        Ok(())
    }
    async fn find_by_org(&self, org: OrgId) -> AppResult<Option<Subscription>> {
        Ok(self.rows.lock().unwrap().get(&org).cloned())
    }
    async fn update(&self, sub: &Subscription) -> AppResult<()> {
        self.rows
            .lock()
            .unwrap()
            .insert(sub.organization_id, sub.clone());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Refresh tokens
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct InMemoryRefreshTokenRepo {
    rows: Mutex<HashMap<RefreshTokenId, RefreshTokenRecord>>,
}

#[async_trait]
impl RefreshTokenRepository for InMemoryRefreshTokenRepo {
    async fn create(&self, rec: &RefreshTokenRecord) -> AppResult<()> {
        self.rows.lock().unwrap().insert(rec.id, rec.clone());
        Ok(())
    }
    async fn find_by_hash(&self, token_hash: &str) -> AppResult<Option<RefreshTokenRecord>> {
        Ok(self
            .rows
            .lock()
            .unwrap()
            .values()
            .find(|r| r.token_hash == token_hash)
            .cloned())
    }
    async fn revoke(&self, id: RefreshTokenId) -> AppResult<()> {
        if let Some(r) = self.rows.lock().unwrap().get_mut(&id) {
            r.revoked_at = Some(Utc::now());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// File storage
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct InMemoryFileStorage {
    blobs: Mutex<HashMap<String, Vec<u8>>>,
}

#[async_trait]
impl FileStorage for InMemoryFileStorage {
    async fn put(&self, key: &str, bytes: &[u8]) -> AppResult<()> {
        self.blobs
            .lock()
            .unwrap()
            .insert(key.to_string(), bytes.to_vec());
        Ok(())
    }
    async fn get(&self, key: &str) -> AppResult<Vec<u8>> {
        self.blobs
            .lock()
            .unwrap()
            .get(key)
            .cloned()
            .ok_or_else(|| not_found("file"))
    }
    async fn delete(&self, key: &str) -> AppResult<()> {
        self.blobs.lock().unwrap().remove(key);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Documents + versions (implements the §5 add_version transaction in-memory)
// ---------------------------------------------------------------------------

pub struct InMemoryDocumentRepo {
    docs: Mutex<HashMap<DocumentId, Document>>,
    versions: Mutex<Vec<DocumentVersion>>,
    machines: Arc<InMemoryMachineRepo>,
}

impl InMemoryDocumentRepo {
    pub fn new(machines: Arc<InMemoryMachineRepo>) -> Self {
        Self {
            docs: Mutex::new(HashMap::new()),
            versions: Mutex::new(Vec::new()),
            machines,
        }
    }
}

#[async_trait]
impl DocumentRepository for InMemoryDocumentRepo {
    async fn create(&self, doc: &Document) -> AppResult<()> {
        self.docs.lock().unwrap().insert(doc.id, doc.clone());
        Ok(())
    }

    async fn find_by_id(&self, org: OrgId, id: DocumentId) -> AppResult<Document> {
        let doc = self
            .docs
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| not_found("document"))?;
        // org scope: the document's machine must belong to org
        self.machines.find_by_id(org, doc.machine_id).await?;
        Ok(doc)
    }

    async fn list_by_machine(
        &self,
        org: OrgId,
        machine: MachineId,
        category: Option<DocumentCategory>,
    ) -> AppResult<Vec<Document>> {
        self.machines.find_by_id(org, machine).await?;
        Ok(self
            .docs
            .lock()
            .unwrap()
            .values()
            .filter(|d| d.machine_id == machine)
            .filter(|d| category.is_none_or(|c| d.category == c))
            .cloned()
            .collect())
    }

    async fn update_meta(&self, org: OrgId, doc: &Document) -> AppResult<()> {
        self.find_by_id(org, doc.id).await?;
        self.docs.lock().unwrap().insert(doc.id, doc.clone());
        Ok(())
    }

    async fn delete(&self, org: OrgId, id: DocumentId) -> AppResult<()> {
        self.find_by_id(org, id).await?;
        self.docs.lock().unwrap().remove(&id);
        self.versions
            .lock()
            .unwrap()
            .retain(|v| v.document_id != id);
        Ok(())
    }

    async fn list_versions(&self, org: OrgId, doc: DocumentId) -> AppResult<Vec<DocumentVersion>> {
        self.find_by_id(org, doc).await?;
        let mut vs: Vec<DocumentVersion> = self
            .versions
            .lock()
            .unwrap()
            .iter()
            .filter(|v| v.document_id == doc)
            .cloned()
            .collect();
        vs.sort_by_key(|v| std::cmp::Reverse(v.version_no)); // newest first
        Ok(vs)
    }

    async fn find_version(
        &self,
        org: OrgId,
        doc: DocumentId,
        version_no: i32,
    ) -> AppResult<DocumentVersion> {
        self.find_by_id(org, doc).await?;
        self.versions
            .lock()
            .unwrap()
            .iter()
            .find(|v| v.document_id == doc && v.version_no == version_no)
            .cloned()
            .ok_or_else(|| not_found("document version"))
    }

    async fn current_version(
        &self,
        org: OrgId,
        doc: DocumentId,
    ) -> AppResult<Option<DocumentVersion>> {
        self.find_by_id(org, doc).await?;
        Ok(self
            .versions
            .lock()
            .unwrap()
            .iter()
            .find(|v| v.document_id == doc && v.is_current)
            .cloned())
    }

    async fn find_version_by_id(&self, id: VersionId) -> AppResult<Option<DocumentVersion>> {
        Ok(self
            .versions
            .lock()
            .unwrap()
            .iter()
            .find(|v| v.id == id)
            .cloned())
    }

    async fn add_version(
        &self,
        org: OrgId,
        doc_id: DocumentId,
        input: NewVersionInput,
    ) -> AppResult<DocumentVersion> {
        let doc = self.find_by_id(org, doc_id).await?;
        let next = doc.current_version_no + 1;
        let now = Utc::now();
        let version = DocumentVersion {
            id: input.id,
            document_id: doc_id,
            version_no: next,
            is_current: true,
            storage_key: input.storage_key,
            original_filename: input.original_filename,
            mime_type: input.mime_type,
            size_bytes: input.size_bytes,
            checksum_sha256: input.checksum_sha256,
            content_json: input.content_json,
            change_note: input.change_note,
            metadata: input.metadata,
            created_by: input.created_by,
            created_at: now,
        };
        {
            let mut versions = self.versions.lock().unwrap();
            for v in versions.iter_mut().filter(|v| v.document_id == doc_id) {
                v.is_current = false;
            }
            versions.push(version.clone());
        }
        {
            let mut docs = self.docs.lock().unwrap();
            if let Some(d) = docs.get_mut(&doc_id) {
                d.current_version_no = next;
                d.updated_at = now;
            }
        }
        if let Some(machine_id) = input.primary_photo_for {
            let mut m = self.machines.find_by_id(org, machine_id).await?;
            m.primary_photo_version_id = Some(input.id);
            self.machines.update(&m).await?;
        }
        Ok(version)
    }
}

// ---------------------------------------------------------------------------
// Security fakes (deterministic, no crypto)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct FakeHasher;

impl PasswordHasher for FakeHasher {
    fn hash(&self, password: &str) -> AppResult<String> {
        Ok(format!("hashed:{password}"))
    }
    fn verify(&self, password: &str, hash: &str) -> AppResult<bool> {
        Ok(hash == format!("hashed:{password}"))
    }
}

#[derive(Default)]
pub struct FakeTokenService;

impl TokenService for FakeTokenService {
    fn issue_access(&self, user_id: UserId, org: OrgId, role: Role) -> AppResult<String> {
        Ok(format!("access:{user_id}:{org}:{}", role.as_str()))
    }
    fn verify_access(&self, token: &str) -> AppResult<AccessClaims> {
        let rest = token
            .strip_prefix("access:")
            .ok_or_else(|| ApplicationError::Unauthorized("bad token".into()))?;
        let parts: Vec<&str> = rest.split(':').collect();
        if parts.len() != 3 {
            return Err(ApplicationError::Unauthorized("bad token".into()));
        }
        let sub = UserId::from_str(parts[0])
            .map_err(|_| ApplicationError::Unauthorized("bad token".into()))?;
        let org = OrgId::from_str(parts[1])
            .map_err(|_| ApplicationError::Unauthorized("bad token".into()))?;
        let role = Role::from_str(parts[2])
            .map_err(|_| ApplicationError::Unauthorized("bad token".into()))?;
        Ok(AccessClaims {
            sub,
            org,
            role,
            iat: 0,
            exp: i64::MAX,
        })
    }
    fn generate_refresh_token(&self) -> String {
        Uuid::now_v7().simple().to_string()
    }
    fn hash_refresh_token(&self, raw: &str) -> String {
        format!("sha256:{raw}")
    }
}

// ---------------------------------------------------------------------------
// World: fakes + real services, wired together
// ---------------------------------------------------------------------------

pub struct World {
    pub orgs: Arc<InMemoryOrgRepo>,
    pub users: Arc<InMemoryUserRepo>,
    pub machines: Arc<InMemoryMachineRepo>,
    pub plans: Arc<InMemoryPlanRepo>,
    pub subs: Arc<InMemorySubscriptionRepo>,
    pub refresh: Arc<InMemoryRefreshTokenRepo>,
    pub storage: Arc<InMemoryFileStorage>,
    pub docs: Arc<InMemoryDocumentRepo>,
    pub hasher: Arc<FakeHasher>,
    pub tokens: Arc<FakeTokenService>,

    pub auth: AuthService,
    pub users_svc: UserService,
    pub orgs_svc: OrganizationService,
    pub machines_svc: MachineService,
    pub docs_svc: DocumentService,
    pub pricing_svc: PricingService,
}

impl World {
    pub fn new() -> Self {
        let orgs = Arc::new(InMemoryOrgRepo::default());
        let users = Arc::new(InMemoryUserRepo::default());
        let machines = Arc::new(InMemoryMachineRepo::default());
        let plans = Arc::new(InMemoryPlanRepo::seeded());
        let subs = Arc::new(InMemorySubscriptionRepo::default());
        let refresh = Arc::new(InMemoryRefreshTokenRepo::default());
        let storage = Arc::new(InMemoryFileStorage::default());
        let docs = Arc::new(InMemoryDocumentRepo::new(machines.clone()));
        let hasher = Arc::new(FakeHasher);
        let tokens = Arc::new(FakeTokenService);

        let auth = AuthService::new(
            users.clone(),
            orgs.clone(),
            subs.clone(),
            refresh.clone(),
            hasher.clone(),
            tokens.clone(),
            900,
            2_592_000,
        );
        let users_svc = UserService::new(users.clone(), hasher.clone());
        let orgs_svc = OrganizationService::new(orgs.clone());
        let machines_svc = MachineService::new(machines.clone(), plans.clone());
        let docs_svc = DocumentService::new(docs.clone(), machines.clone(), storage.clone());
        let pricing_svc = PricingService::new(machines.clone(), plans.clone(), subs.clone());

        Self {
            orgs,
            users,
            machines,
            plans,
            subs,
            refresh,
            storage,
            docs,
            hasher,
            tokens,
            auth,
            users_svc,
            orgs_svc,
            machines_svc,
            docs_svc,
            pricing_svc,
        }
    }
}
