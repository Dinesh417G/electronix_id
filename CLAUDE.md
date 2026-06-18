# ElectronIx ID — Backend (`electronix-id-api`)

> Instructions for Claude Code. Build the backend for the **Machine Digital Passport**:
> a multi-tenant REST API that manages organizations, users, machines, and a **generic,
> version-controlled document store** (photos, BOM, manuals, PLC/CNC/any programs, specs),
> plus authentication and per-machine pricing tiers.
>
> **This milestone is backend only.** No frontend, no edge agent, no MQTT. Build it well,
> layer it cleanly, test every layer.

---

## 1. Golden rules (read before writing any code)

1. **Do not invent crate versions.** Use the exact versions in §6. If a version genuinely does not build, stop and report it — do not silently bump.
2. **Layered architecture is mandatory.** Dependencies point *inward only*: `web → application → domain`, and `infrastructure → application + domain`. The `domain` layer depends on nothing but std + `uuid` + `chrono`. Business logic lives in `application` services, never in handlers, never in repositories.
3. **The database is touched only through repository traits.** Services depend on traits (ports), not on `sqlx`. Only `infrastructure` knows MySQL exists.
4. **Every query is scoped by `organization_id`.** This is a multi-tenant system. A user from org A must never read or write org B's data. Treat a missing org scope as a security bug.
5. **JSON is `snake_case` everywhere** (serde default — do not add `rename_all = "camelCase"`). The frontend will match the backend. We are deliberately avoiding the camelCase/snake_case mismatch that bit the CNC tool.
6. **Build incrementally in the milestone order in §13.** After each milestone: `cargo clippy --all-targets -- -D warnings`, `cargo fmt`, `cargo test` must all pass before moving on.
7. **Money is integer minor units** (paise) + a 3-letter currency code. No floats for money, ever.
8. **All timestamps are UTC**, stored as `DATETIME(6)`, serialized as RFC 3339.

### Mental model (for the human reading this)
- A **repository trait** is a hardware-abstraction layer: the service (the OB program) calls `find_by_id` the same way regardless of whether the backing store is MySQL today or something else later.
- The **document/version split** is exactly like a PLC program archive: `documents` is the program slot ("Main OB1"), `document_versions` are the dated backups you never delete — you always know what changed and you can reload an old one.

---

## 2. Scope of this build

**In scope**
- Organizations, Users (with roles), Machines.
- Generic versioned documents: any artifact on a machine — photo, BOM, mechanical manual, electrical manual, PLC program, CNC program, robot/HMI program, VFD parameters, drawings, datasheets, **specifications**, certificates, maintenance records, or `other`. Files *and* structured JSON, both versioned.
- Authentication (argon2 password hashing + JWT access/refresh) and RBAC.
- Pricing: plan catalog (Basic / Live / Predict), per-machine tier, org subscription record, and a cost-estimate service.
- All four application layers, wired and tested.

**Out of scope (future crates — leave room, build nothing)**
- `resolver`, `edge-agent`, `ingest`, `taggen` crates; MQTT; live telemetry / LIVE-tier data path.
- Payments/Razorpay/Stripe integration (model the data, do not integrate a gateway).
- Cloudflare R2 wiring (implement the `FileStorage` trait + local-disk impl now; R2 drops in behind the same trait later).
- Next.js UI, Tauri field app, email/invite sending, audit log, rate limiting.

---

## 3. Tech stack

| Concern | Choice |
|---|---|
| Language / edition | Rust, edition **2024**, `rust-version = "1.85"` |
| HTTP framework | **Axum 0.8** |
| Async runtime | Tokio 1 (`full`) |
| Database | **MySQL 8**, accessed via **sqlx 0.8** (compile-time-checked queries) |
| Migrations | `sqlx migrate` (`./migrations`, timestamped `.sql`) |
| Auth | `argon2` (Argon2id) + `jsonwebtoken` (HS256) |
| Validation | `validator` (derive) |
| Errors | `thiserror` in libs, `anyhow` only in `main` |
| IDs | UUID **v7** (time-sortable), stored as `CHAR(36)` |
| Logging | `tracing` + `tracing-subscriber` + `tower-http` `TraceLayer` |
| File storage | `FileStorage` trait; `LocalFileStorage` (tokio::fs) now |

---

## 4. Workspace & directory layout

Set up a Cargo **workspace** (so `resolver`/`ingest`/`edge-agent`/`shared` slot in later) with a single member for now: `crates/api`. Inside `api`, the layers are modules.

> **Do not** split a `shared` crate yet — there is no second consumer. Extract it when the edge agent arrives. Don't abstract before there's a reason.

```
electronix-id/
├── Cargo.toml                  # [workspace] members = ["crates/api"]
├── .gitignore                  # target/, .env, /storage, .sqlx is COMMITTED
├── .env.example
├── README.md
├── migrations/                 # 0001_init.sql, 0002_..., owned by sqlx
└── crates/
    └── api/
        ├── Cargo.toml          # name = "electronix-id-api", bin "api"
        └── src/
            ├── main.rs         # load config, init tracing, pool, migrate, build router, serve
            ├── config.rs       # Settings::from_env()
            ├── state.rs        # AppState { services... }
            ├── error.rs        # AppError + IntoResponse + From conversions
            │
            ├── domain/         # PURE. no sqlx, no axum.
            │   ├── mod.rs
            │   ├── ids.rs      # newtypes: OrgId, UserId, MachineId, DocumentId, VersionId (wrap Uuid)
            │   ├── value_objects.rs  # Email, Role, DocumentCategory, StorageKind, MachineStatus, Tier, Money
            │   ├── organization.rs
            │   ├── user.rs
            │   ├── machine.rs
            │   ├── document.rs # Document, DocumentVersion + versioning invariants
            │   ├── plan.rs     # Plan, Subscription, SubscriptionStatus
            │   └── error.rs    # DomainError
            │
            ├── application/    # use cases + ports. depends on domain only.
            │   ├── mod.rs
            │   ├── ports/      # repository + infra TRAITS (async_trait)
            │   │   ├── mod.rs
            │   │   ├── organization_repo.rs
            │   │   ├── user_repo.rs
            │   │   ├── machine_repo.rs
            │   │   ├── document_repo.rs
            │   │   ├── plan_repo.rs
            │   │   ├── password_hasher.rs
            │   │   ├── token_service.rs
            │   │   └── file_storage.rs
            │   ├── auth_service.rs
            │   ├── user_service.rs
            │   ├── organization_service.rs
            │   ├── machine_service.rs
            │   ├── document_service.rs   # owns the version-bump transaction logic
            │   ├── pricing_service.rs
            │   └── error.rs              # ApplicationError
            │
            ├── infrastructure/ # adapters. implements application::ports.
            │   ├── mod.rs
            │   ├── db.rs                 # MySqlPool builder
            │   ├── persistence/
            │   │   ├── mod.rs
            │   │   ├── mysql_organization_repo.rs
            │   │   ├── mysql_user_repo.rs
            │   │   ├── mysql_machine_repo.rs
            │   │   ├── mysql_document_repo.rs
            │   │   └── mysql_plan_repo.rs
            │   ├── security/
            │   │   ├── mod.rs
            │   │   ├── argon2_hasher.rs
            │   │   └── jwt_token_service.rs
            │   └── storage/
            │       ├── mod.rs
            │       └── local_file_storage.rs
            │
            └── web/            # axum. depends on application + domain.
                ├── mod.rs
                ├── router.rs            # build_router(AppState) -> Router
                ├── middleware.rs        # request-id, trace, CORS, body limit
                ├── extractors.rs        # AuthUser, ValidatedJson<T>
                ├── dto.rs               # request/response structs (serde)
                ├── pagination.rs        # Page<T>, PageParams
                └── handlers/
                    ├── mod.rs
                    ├── health.rs
                    ├── auth.rs
                    ├── users.rs
                    ├── organization.rs
                    ├── machines.rs
                    ├── documents.rs
                    └── pricing.rs
```

---

## 5. Data model (MySQL 8)

Write this as `migrations/0001_init.sql`. Engine InnoDB, charset `utf8mb4`. IDs are `CHAR(36)` UUIDv7. Money columns are `BIGINT` paise.

```sql
-- organizations
CREATE TABLE organizations (
  id          CHAR(36)     NOT NULL PRIMARY KEY,
  name        VARCHAR(160) NOT NULL,
  slug        VARCHAR(160) NOT NULL UNIQUE,
  created_at  DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at  DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- users  (email is globally unique; login resolves the org)
CREATE TABLE users (
  id              CHAR(36)     NOT NULL PRIMARY KEY,
  organization_id CHAR(36)     NOT NULL,
  email           VARCHAR(255) NOT NULL UNIQUE,
  password_hash   VARCHAR(255) NOT NULL,
  full_name       VARCHAR(160) NOT NULL,
  role            VARCHAR(16)  NOT NULL,              -- owner|admin|engineer|viewer
  is_active       BOOLEAN      NOT NULL DEFAULT TRUE,
  created_at      DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at      DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  KEY idx_users_org (organization_id),
  CONSTRAINT fk_users_org FOREIGN KEY (organization_id)
    REFERENCES organizations(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- refresh tokens (rotation + revocation; store SHA-256 hash, never the raw token)
CREATE TABLE refresh_tokens (
  id         CHAR(36)    NOT NULL PRIMARY KEY,
  user_id    CHAR(36)    NOT NULL,
  token_hash CHAR(64)    NOT NULL UNIQUE,             -- sha256 hex
  expires_at DATETIME(6) NOT NULL,
  revoked_at DATETIME(6) NULL,
  created_at DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  KEY idx_rt_user (user_id),
  CONSTRAINT fk_rt_user FOREIGN KEY (user_id)
    REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- plans (pricing catalog)
CREATE TABLE plans (
  id                     CHAR(36)    NOT NULL PRIMARY KEY,
  code                   VARCHAR(16) NOT NULL UNIQUE,  -- basic|live|predict
  name                   VARCHAR(80) NOT NULL,
  price_per_machine_year BIGINT      NOT NULL,         -- paise
  onboarding_fee         BIGINT      NOT NULL DEFAULT 0,-- paise, one-time per machine
  currency               CHAR(3)     NOT NULL DEFAULT 'INR',
  features               JSON        NULL,
  is_active              BOOLEAN     NOT NULL DEFAULT TRUE,
  created_at             DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- subscription (one per org; tracks status + billing period)
CREATE TABLE subscriptions (
  id                   CHAR(36)    NOT NULL PRIMARY KEY,
  organization_id      CHAR(36)    NOT NULL UNIQUE,
  status               VARCHAR(16) NOT NULL,           -- trialing|active|past_due|canceled
  trial_ends_at        DATETIME(6) NULL,
  current_period_start DATETIME(6) NULL,
  current_period_end   DATETIME(6) NULL,
  created_at           DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at           DATETIME(6) NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  CONSTRAINT fk_sub_org FOREIGN KEY (organization_id)
    REFERENCES organizations(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- machines  (core identity is mutable; rich specs live in versioned documents)
CREATE TABLE machines (
  id                       CHAR(36)     NOT NULL PRIMARY KEY,
  organization_id          CHAR(36)     NOT NULL,
  plan_id                  CHAR(36)     NULL,          -- this machine's pricing tier
  name                     VARCHAR(160) NOT NULL,
  make                     VARCHAR(120) NULL,
  model                    VARCHAR(120) NULL,
  serial_number            VARCHAR(120) NULL,
  asset_tag                VARCHAR(64)  NULL,
  location                 VARCHAR(160) NULL,
  year_installed           SMALLINT     NULL,
  status                   VARCHAR(16)  NOT NULL DEFAULT 'active', -- active|maintenance|retired
  primary_photo_version_id CHAR(36)     NULL,          -- convenience pointer
  created_by               CHAR(36)     NULL,
  created_at               DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at               DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  KEY idx_machines_org (organization_id),
  CONSTRAINT fk_machines_org  FOREIGN KEY (organization_id)
    REFERENCES organizations(id) ON DELETE CASCADE,
  CONSTRAINT fk_machines_plan FOREIGN KEY (plan_id)
    REFERENCES plans(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- documents : a logical, versioned artifact slot on a machine (THE generic part)
CREATE TABLE documents (
  id                 CHAR(36)     NOT NULL PRIMARY KEY,
  machine_id         CHAR(36)     NOT NULL,
  category           VARCHAR(40)  NOT NULL,   -- see DocumentCategory enum
  name               VARCHAR(200) NOT NULL,   -- human label (required when category=other)
  storage_kind       VARCHAR(8)   NOT NULL,   -- file | json
  current_version_no INT          NOT NULL DEFAULT 0,
  created_by         CHAR(36)     NULL,
  created_at         DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  updated_at         DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6) ON UPDATE CURRENT_TIMESTAMP(6),
  KEY idx_documents_machine (machine_id),
  KEY idx_documents_machine_cat (machine_id, category),
  CONSTRAINT fk_documents_machine FOREIGN KEY (machine_id)
    REFERENCES machines(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

-- document_versions : immutable snapshots. INSERT only — never UPDATE the payload.
CREATE TABLE document_versions (
  id                CHAR(36)     NOT NULL PRIMARY KEY,
  document_id       CHAR(36)     NOT NULL,
  version_no        INT          NOT NULL,   -- 1,2,3...
  is_current        BOOLEAN      NOT NULL DEFAULT FALSE,
  -- file payload (when documents.storage_kind = 'file')
  storage_key       VARCHAR(512) NULL,
  original_filename VARCHAR(255) NULL,
  mime_type         VARCHAR(160) NULL,
  size_bytes        BIGINT       NULL,
  checksum_sha256   CHAR(64)     NULL,
  -- json payload (when documents.storage_kind = 'json'): specs, parameters, BOM rows
  content_json      JSON         NULL,
  -- common
  change_note       VARCHAR(500) NULL,        -- "what changed" / changelog line
  metadata          JSON         NULL,        -- typed extras: {o_number, controller, plc_model,...}
  created_by        CHAR(36)     NULL,
  created_at        DATETIME(6)  NOT NULL DEFAULT CURRENT_TIMESTAMP(6),
  UNIQUE KEY uq_docver (document_id, version_no),
  KEY idx_docver_current (document_id, is_current),
  CONSTRAINT fk_docver_document FOREIGN KEY (document_id)
    REFERENCES documents(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
```

Seed the plans in `migrations/0002_seed_plans.sql` (prices from the product doc; adjust if asked):

```sql
INSERT INTO plans (id, code, name, price_per_machine_year, onboarding_fee, currency, features, is_active) VALUES
 (UUID(), 'basic',   'Passport Basic',   150000,  60000, 'INR',
   JSON_OBJECT('static_passport', true, 'live_data', false, 'predict', false), TRUE),
 (UUID(), 'live',    'Passport Live',    360000,  60000, 'INR',
   JSON_OBJECT('static_passport', true, 'live_data', true,  'predict', false), TRUE),
 (UUID(), 'predict', 'Passport Predict', 0,       60000, 'INR',
   JSON_OBJECT('static_passport', true, 'live_data', true,  'predict', true),  FALSE);
-- prices in paise: 150000 = ₹1,500/yr ; 360000 = ₹3,600/yr ; onboarding 60000 = ₹600.
-- Predict price 0 + is_active FALSE = roadmap placeholder.
```

> `UUID()` is fine for seed rows. **All app-created rows use UUIDv7 generated in Rust** (`Uuid::now_v7()`), bound as `.to_string()`, read back via the `uuid` sqlx feature.

### Versioning logic (the core requirement) — `DocumentService`, in one transaction
Adding/updating an artifact never overwrites. To create a new version:
1. `BEGIN`; `SELECT ... FOR UPDATE` the `documents` row (serialize concurrent uploads).
2. `next = current_version_no + 1`.
3. For `file` kind: store bytes via `FileStorage`, compute `size_bytes` + `checksum_sha256`; for `json` kind: validate it is an object/array. `INSERT` the new `document_versions` row with `version_no = next`, `is_current = TRUE`.
4. `UPDATE document_versions SET is_current = FALSE WHERE document_id = ? AND version_no < next`.
5. `UPDATE documents SET current_version_no = next`.
6. If `category = 'photo'`, `UPDATE machines.primary_photo_version_id`.
7. `COMMIT`.

**Restore** an old version = read that version's payload and run the same flow to create a *new* highest version that copies it (`change_note = "restored from v{n}"`). Lineage is preserved; nothing is deleted.

`DocumentCategory` (Rust enum; stored as the snake_case string; unknown/custom → `Other`):
`photo, bom, mechanical_manual, electrical_manual, plc_program, cnc_program, robot_program, hmi_program, vfd_parameters, parameter_backup, electrical_drawing, mechanical_drawing, datasheet, specification, certificate, maintenance_record, other`.

---

## 6. `crates/api/Cargo.toml` dependencies (pin these)

```toml
[package]
name = "electronix-id-api"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[[bin]]
name = "api"
path = "src/main.rs"

[dependencies]
axum = { version = "0.8", features = ["macros", "multipart"] }
tokio = { version = "1", features = ["full"] }
tower = { version = "0.5", features = ["util"] }
tower-http = { version = "0.6", features = ["trace", "cors", "request-id", "util", "limit"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "mysql", "macros", "migrate", "chrono", "uuid", "json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v7", "serde"] }
argon2 = "0.5"
jsonwebtoken = "9"
validator = { version = "0.19", features = ["derive"] }
thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
async-trait = "0.1"
sha2 = "0.10"
rand = "0.8"
dotenvy = "0.15"

[dev-dependencies]
http-body-util = "0.1"
```

> Async traits behind `Arc<dyn Repository>` are not dyn-safe with native `async fn` yet → use `#[async_trait]` on all port traits. Services hold `Arc<dyn ...>`; `AppState` holds the services.

---

## 7. API surface (all under `/api/v1`, JSON, `snake_case`)

**Auth** (public except `me`)
- `POST /auth/register` → `{ organization_name, email, password, full_name }` creates org + owner user + a `trialing` subscription. Returns tokens.
- `POST /auth/login` → `{ email, password }` → `{ access_token, refresh_token, expires_in }`.
- `POST /auth/refresh` → `{ refresh_token }` → new access token; **rotate** the refresh token (revoke old, issue new).
- `POST /auth/logout` → revokes the presented refresh token.
- `GET /auth/me` → current user + organization.

**Users** (org-scoped; create/modify = admin/owner)
- `GET /users` · `POST /users` · `GET /users/{id}` · `PATCH /users/{id}` · `DELETE /users/{id}`

**Organization**
- `GET /organization` · `PATCH /organization`

**Machines** (org-scoped, paginated)
- `GET /machines` · `POST /machines` · `GET /machines/{id}` · `PATCH /machines/{id}` · `DELETE /machines/{id}`
- `PATCH /machines/{id}/tier` → `{ plan_code }` (admin/owner)

**Documents & versions** (generic, versioned)
- `GET  /machines/{machine_id}/documents` (filter `?category=`)
- `POST /machines/{machine_id}/documents` → create a slot `{ category, name, storage_kind }` (optionally with first version)
- `GET  /documents/{document_id}` → slot + current version + version list
- `PATCH /documents/{document_id}` → rename / recategorize
- `DELETE /documents/{document_id}`
- `POST /documents/{document_id}/versions` → **new version**. `multipart/form-data` (file + `change_note`, `metadata`) when `storage_kind=file`; `application/json` body when `storage_kind=json`.
- `GET  /documents/{document_id}/versions` → history (newest first)
- `GET  /documents/{document_id}/versions/{version_no}` → metadata
- `GET  /documents/{document_id}/versions/{version_no}/download` → stream bytes (local); later returns a presigned URL (R2)
- `POST /documents/{document_id}/versions/{version_no}/restore` → makes a copy the new current version

**Pricing**
- `GET /plans` → catalog
- `GET /organization/subscription` → status + period
- `GET /organization/billing/estimate` → `PricingService` output: per-machine breakdown + totals (annual recurring = Σ active machines' `price_per_machine_year`; plus one-time onboarding for machines added this period). Money returned as `{ amount_minor, currency }`.

**Health**
- `GET /health` (liveness) · `GET /health/ready` (DB `SELECT 1`)

### Response conventions
- List endpoints return `{ "data": [...], "page": 1, "per_page": 20, "total": 137 }`.
- Errors return `{ "error": { "code": "not_found", "message": "..." } }` with the right HTTP status.
- Money is `{ "amount_minor": 150000, "currency": "INR" }`.

---

## 8. Auth & RBAC

- **Passwords:** Argon2id via `argon2`. Never log or return hashes.
- **Access token:** JWT HS256, short-lived (default 15 min). Claims: `sub` (user id), `org` (organization id), `role`, `exp`, `iat`. Secret from `JWT_SECRET` (reject startup if < 32 bytes).
- **Refresh token:** opaque random 32 bytes, hex-encoded, returned to client; only its SHA-256 hash is stored. Default TTL 30 days. Rotate on every refresh (revoke the used one). Logout revokes.
- **`AuthUser` extractor:** validates the bearer JWT, loads the user, exposes `{ user_id, organization_id, role }`. Protected handlers take `AuthUser` as an argument.
- **Roles:** `owner` (all incl. org/billing) ⊃ `admin` (users, machines, documents, tiers) ⊃ `engineer` (create/update machines, documents, versions) ⊃ `viewer` (read-only). Provide a `require_role(min)` check used in handlers/services. Org isolation is enforced in **every** repository method by taking `organization_id` and filtering on it.

---

## 9. Pricing model (application layer)

- `plans` is the catalog. Each **machine** carries a `plan_id` (its tier). The **org** has one `subscriptions` row holding status + billing period.
- `PricingService::estimate(org_id)`:
  - recurring annual = Σ over `status='active'` machines of their plan's `price_per_machine_year`;
  - one-time = Σ `onboarding_fee` for machines created within the current period (or all machines if no period set);
  - returns a per-machine line breakdown + totals, all in minor units.
- Provide a `tier_allows(machine, feature)` helper (e.g. `live_data`) so future LIVE endpoints can be gated. Do not build the live endpoints now.

---

## 10. Configuration (`Settings::from_env`, via `dotenvy`)

`.env.example`:
```
DATABASE_URL=mysql://root:password@localhost:3306/electronix_id
# If reusing the existing MySQL container, it may be on port 3308.
BIND_ADDR=0.0.0.0:8080
JWT_SECRET=change-me-to-at-least-32-bytes-of-random
ACCESS_TOKEN_TTL_SECS=900
REFRESH_TOKEN_TTL_SECS=2592000
STORAGE_ROOT=./storage
MAX_UPLOAD_BYTES=52428800
RUST_LOG=info,electronix_id_api=debug,sqlx=warn
CORS_ALLOWED_ORIGINS=http://localhost:3000
```
`main.rs` order: load `.env` → init tracing (`EnvFilter` from `RUST_LOG`) → build `MySqlPool` → `sqlx::migrate!().run(&pool)` → construct infra adapters → construct services → `AppState` → `build_router` → bind + `axum::serve`.

---

## 11. Build / run / test commands

```bash
# one-time: sqlx CLI for migrations + offline prepare
cargo install sqlx-cli --no-default-features --features rustls,mysql

# create DB and run migrations
sqlx database create
sqlx migrate run

# run the server
cargo run

# BEFORE COMMITTING: regenerate the offline query cache so CI builds without a DB
cargo sqlx prepare --workspace        # writes/updates ./.sqlx  (commit this)

# quality gates (must be clean before finishing each milestone)
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

> Use the `query!` / `query_as!` macros so SQL is checked at compile time against the live schema, then commit `./.sqlx` so the project builds offline. `.env` and `/storage` are git-ignored; `.sqlx` is committed.

---

## 12. Coding conventions & testing

- **No business logic in handlers.** A handler: extract + validate input → call one service method → map the result to a response. That's it.
- **Repositories are thin.** They map rows ↔ domain types and run SQL. No business rules.
- **Errors:** `DomainError` (invariants) and `ApplicationError` (`NotFound`, `Unauthorized`, `Forbidden`, `Conflict`, `Validation`, `Internal`). `infrastructure` maps `sqlx::Error::RowNotFound → NotFound`, others → `Internal` (logged with `tracing`). `web::error` implements `IntoResponse` for the error type → status + JSON envelope. Never leak SQL or internal messages to clients.
- **Validation:** derive `validator::Validate` on request DTOs; the `ValidatedJson<T>` extractor returns a 422 with field errors on failure.
- **Uploads:** enforce `MAX_UPLOAD_BYTES` (also via `tower-http` `RequestBodyLimitLayer`); record `mime_type`, `size_bytes`, `checksum_sha256`; store files under `STORAGE_ROOT/{org_id}/{machine_id}/{document_id}/{version_no}/{filename}`.
- **Tests per layer:**
  - *domain* — pure unit tests for invariants (e.g. invalid email rejected, version_no monotonic helper).
  - *services* — hand-written in-memory fakes implementing the port traits (no DB, no mockall). Test version-bump, restore, RBAC, org-scoping, pricing math.
  - *repositories* — `#[sqlx::test]` integration tests (each gets an isolated DB / auto-rollback). Cover CRUD + the document version transaction.
  - *handlers* — `tower::ServiceExt::oneshot` against the `Router`; cover auth flow (register → login → protected route → refresh → logout) and **cross-org isolation** (org A token cannot read org B's machine → 404/403).
- **Tracing:** `TraceLayer` + a request-id layer; log at info for requests, debug for service steps, error for `Internal`.

---

## 13. Build plan — do these in order, gate each on its acceptance criteria

**M0 — Scaffold.** Workspace, `api` crate with §6 deps, `config.rs`, tracing, `MySqlPool`, migrations runner, `/health` + `/health/ready`, `.env.example`, `.gitignore`, `README`.
*AC:* `cargo run` serves `GET /health` → 200; `sqlx migrate run` succeeds.

**M1 — Domain + errors.** Id newtypes, value objects, entities, `DomainError`. Pure.
*AC:* `domain` compiles with no sqlx/axum import; unit tests pass.

**M2 — Persistence foundation.** `0001_init.sql` + `0002_seed_plans.sql`; port traits; MySQL repo impls for organizations, users, plans; `AppState` skeleton.
*AC:* `cargo sqlx prepare` succeeds; `#[sqlx::test]` CRUD tests pass for organizations + users; plans seed visible.

**M3 — Auth.** Argon2 hasher, JWT service, refresh-token table use + rotation, `AuthUser` extractor, `require_role`, register/login/refresh/logout/me.
*AC:* handler test register → login → call `/auth/me` → refresh → logout passes; expired/invalid tokens rejected.

**M4 — Users & Organization.** Management endpoints, org scoping, role checks.
*AC:* viewer is forbidden from writes; user from org A cannot touch org B (tests).

**M5 — Machines.** CRUD, `PATCH /tier`, pagination, org scoping, `created_by`.
*AC:* cross-org isolation test; tier change reflects in machine; pagination envelope correct.

**M6 — Generic versioned documents.** `documents` + `document_versions`; `FileStorage` trait + `LocalFileStorage`; create slot; new version (multipart for file, JSON for json); history; download (stream); restore. Implement the §5 transaction exactly.
*AC:* upload v1 then v2 → history shows 2, current = v2; restore v1 → creates v3 (current) copying v1; download returns stored bytes; checksum matches; `category=photo` updates `primary_photo_version_id`.

**M7 — Pricing.** Subscription created on register (`trialing`); `GET /plans`; `GET /organization/subscription`; `GET /organization/billing/estimate` via `PricingService`; `tier_allows` helper.
*AC:* estimate equals Σ active machines' plan price + onboarding; unit tests on the math; Predict plan excluded from active recurring (price 0 / inactive).

**M8 — Hardening.** Complete error→HTTP mapping, validation on all DTOs, CORS, request-id + trace, body-size limit, consistent envelopes. (Optional: `utoipa` OpenAPI — only if time allows; clearly optional.)
*AC:* `cargo clippy --all-targets -- -D warnings` clean; `cargo fmt --check` clean; `cargo test` green; `./.sqlx` committed; README run steps verified.

---

## 14. Definition of done (this milestone)

- All of M0–M8 complete with their acceptance criteria met.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` all pass.
- `./.sqlx` committed so the project builds without a live DB.
- `migrations/` creates the full schema and seeds the three plans.
- README documents env vars and the run/test commands.
- Layer boundaries respected: no `sqlx` outside `infrastructure`, no business logic in `web`, `domain` depends on nothing external.

When the backend is green, stop and report status with the list of endpoints implemented and any deviations. The next milestone (separate) will add the `resolver` crate and the QR scan path.
