//! Authentication + registration use cases, plus the shared `require_role`
//! RBAC gate used across services and handlers.

use std::sync::Arc;

use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::application::error::{AppResult, ApplicationError};
use crate::application::ports::organization_repo::OrganizationRepository;
use crate::application::ports::password_hasher::PasswordHasher;
use crate::application::ports::refresh_token_repo::{RefreshTokenRecord, RefreshTokenRepository};
use crate::application::ports::subscription_repo::SubscriptionRepository;
use crate::application::ports::token_service::TokenService;
use crate::application::ports::user_repo::UserRepository;
use crate::domain::ids::{OrgId, RefreshTokenId, SubscriptionId, UserId};
use crate::domain::organization::Organization;
use crate::domain::plan::{Subscription, SubscriptionStatus};
use crate::domain::user::User;
use crate::domain::value_objects::{Email, Role};

/// RBAC gate. `Ok` if `role` is at least `min`, else `Forbidden`.
pub fn require_role(role: Role, min: Role) -> AppResult<()> {
    if role.at_least(min) {
        Ok(())
    } else {
        Err(ApplicationError::Forbidden(format!(
            "requires {} role or higher",
            min.as_str()
        )))
    }
}

/// What a successful auth hands back to the client.
#[derive(Debug, Clone)]
pub struct AuthTokens {
    pub access_token: String,
    /// Raw opaque refresh token — only its hash is stored server-side.
    pub refresh_token: String,
    pub expires_in: i64,
}

#[derive(Clone)]
pub struct AuthService {
    users: Arc<dyn UserRepository>,
    orgs: Arc<dyn OrganizationRepository>,
    subs: Arc<dyn SubscriptionRepository>,
    refresh_tokens: Arc<dyn RefreshTokenRepository>,
    hasher: Arc<dyn PasswordHasher>,
    tokens: Arc<dyn TokenService>,
    access_ttl_secs: i64,
    refresh_ttl_secs: i64,
}

impl AuthService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        users: Arc<dyn UserRepository>,
        orgs: Arc<dyn OrganizationRepository>,
        subs: Arc<dyn SubscriptionRepository>,
        refresh_tokens: Arc<dyn RefreshTokenRepository>,
        hasher: Arc<dyn PasswordHasher>,
        tokens: Arc<dyn TokenService>,
        access_ttl_secs: i64,
        refresh_ttl_secs: i64,
    ) -> Self {
        Self {
            users,
            orgs,
            subs,
            refresh_tokens,
            hasher,
            tokens,
            access_ttl_secs,
            refresh_ttl_secs,
        }
    }

    /// Create org + owner user + a `trialing` subscription, then issue tokens.
    pub async fn register(
        &self,
        organization_name: String,
        email: String,
        password: String,
        full_name: String,
    ) -> AppResult<(User, Organization, AuthTokens)> {
        let email = Email::parse(&email)?;
        if self.users.exists_by_email(&email).await? {
            return Err(ApplicationError::Conflict(
                "email already registered".into(),
            ));
        }

        let now = Utc::now();
        let slug = self
            .unique_slug(Organization::slugify(&organization_name))
            .await?;
        let org = Organization {
            id: OrgId::new(),
            name: organization_name,
            slug,
            created_at: now,
            updated_at: now,
        };
        self.orgs.create(&org).await?;

        let user = User {
            id: UserId::new(),
            organization_id: org.id,
            email,
            password_hash: self.hasher.hash(&password)?,
            full_name,
            role: Role::Owner,
            is_active: true,
            created_at: now,
            updated_at: now,
        };
        self.users.create(&user).await?;

        let sub = Subscription {
            id: SubscriptionId::new(),
            organization_id: org.id,
            status: SubscriptionStatus::Trialing,
            trial_ends_at: Some(now + Duration::days(14)),
            current_period_start: Some(now),
            current_period_end: Some(now + Duration::days(365)),
            created_at: now,
            updated_at: now,
        };
        self.subs.create(&sub).await?;

        let tokens = self.issue_tokens(&user).await?;
        Ok((user, org, tokens))
    }

    pub async fn login(&self, email: String, password: String) -> AppResult<AuthTokens> {
        // Same error for every failure mode — don't leak which part was wrong.
        let invalid = || ApplicationError::Unauthorized("invalid credentials".into());

        let email = Email::parse(&email).map_err(|_| invalid())?;
        let user = self
            .users
            .find_by_email(&email)
            .await?
            .ok_or_else(invalid)?;
        if !user.is_active {
            return Err(invalid());
        }
        if !self.hasher.verify(&password, &user.password_hash)? {
            return Err(invalid());
        }
        self.issue_tokens(&user).await
    }

    /// Validate the presented refresh token, rotate it (revoke old + issue new),
    /// and return a fresh access token too.
    pub async fn refresh(&self, raw_refresh: String) -> AppResult<AuthTokens> {
        let hash = self.tokens.hash_refresh_token(&raw_refresh);
        let rec = self
            .refresh_tokens
            .find_by_hash(&hash)
            .await?
            .ok_or_else(|| ApplicationError::Unauthorized("invalid refresh token".into()))?;

        if !rec.is_usable(Utc::now()) {
            return Err(ApplicationError::Unauthorized(
                "refresh token expired or revoked".into(),
            ));
        }

        self.refresh_tokens.revoke(rec.id).await?;
        let user = self.users.find_by_id_any(rec.user_id).await?;
        self.issue_tokens(&user).await
    }

    /// Idempotent: revoke the presented refresh token if it exists.
    pub async fn logout(&self, raw_refresh: String) -> AppResult<()> {
        let hash = self.tokens.hash_refresh_token(&raw_refresh);
        if let Some(rec) = self.refresh_tokens.find_by_hash(&hash).await? {
            self.refresh_tokens.revoke(rec.id).await?;
        }
        Ok(())
    }

    /// Validate a bearer access token and load the live user behind it. Returns
    /// the identity the web layer attaches to a request (`AuthUser`). A disabled
    /// or vanished account is rejected even with an otherwise-valid token.
    pub async fn authenticate(&self, access_token: &str) -> AppResult<(UserId, OrgId, Role)> {
        let claims = self.tokens.verify_access(access_token)?;
        let user = self.users.find_by_id(claims.org, claims.sub).await?;
        if !user.is_active {
            return Err(ApplicationError::Unauthorized("account is disabled".into()));
        }
        Ok((user.id, user.organization_id, user.role))
    }

    /// Current user + their organization (for `GET /auth/me`).
    pub async fn me(&self, org: OrgId, user_id: UserId) -> AppResult<(User, Organization)> {
        let user = self.users.find_by_id(org, user_id).await?;
        let organization = self.orgs.find_by_id(org).await?;
        Ok((user, organization))
    }

    async fn issue_tokens(&self, user: &User) -> AppResult<AuthTokens> {
        let access = self
            .tokens
            .issue_access(user.id, user.organization_id, user.role)?;

        let raw = self.tokens.generate_refresh_token();
        let now = Utc::now();
        let rec = RefreshTokenRecord {
            id: RefreshTokenId::new(),
            user_id: user.id,
            token_hash: self.tokens.hash_refresh_token(&raw),
            expires_at: now + Duration::seconds(self.refresh_ttl_secs),
            revoked_at: None,
            created_at: now,
        };
        self.refresh_tokens.create(&rec).await?;

        Ok(AuthTokens {
            access_token: access,
            refresh_token: raw,
            expires_in: self.access_ttl_secs,
        })
    }

    async fn unique_slug(&self, base: String) -> AppResult<String> {
        let base = if base.is_empty() {
            "org".to_string()
        } else {
            base
        };
        if self.orgs.find_by_slug(&base).await?.is_none() {
            return Ok(base);
        }
        for _ in 0..5 {
            let suffix = &Uuid::now_v7().simple().to_string()[..6];
            let cand = format!("{base}-{suffix}");
            if self.orgs.find_by_slug(&cand).await?.is_none() {
                return Ok(cand);
            }
        }
        Err(ApplicationError::Conflict(
            "could not allocate a unique organization slug".into(),
        ))
    }
}
