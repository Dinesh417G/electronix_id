# ElectronIx ID — Backend (`electronix-id-api`)

Multi-tenant REST API for the **Machine Digital Passport**: organizations, users,
machines, and a generic, version-controlled document store (photos, BOM, manuals,
PLC/CNC/robot programs, specs), plus auth and per-machine pricing tiers.

Backend only. Layered architecture: `web → application → domain`, with
`infrastructure → application + domain`. The database is touched only through
repository traits; every query is scoped by `organization_id`.

## Layout

```
electronix-id/
├── Cargo.toml          # workspace: crates/api, crates/resolver
├── migrations/         # sqlx migrations (0001_init.sql, ...)
├── crates/api/         # tenant API binary; layers are modules under src/
│   └── src/{domain, application, infrastructure, web}
└── crates/resolver/    # public QR/scan service; reuses api's domain + adapters
```

Two binaries share one database and JWT secret:
- **`api`** (`:8080`) — the authenticated tenant API; owns the schema/migrations.
- **`resolver`** (`:8081`) — a separate **public** service that maps a machine's
  opaque QR tag code to a passport view. A scan needs no login for the summary;
  the full document inventory is gated on a token whose org owns the machine.

## Prerequisites

- Rust 1.85+ (edition 2024)
- MySQL 8
- `sqlx-cli`:
  ```bash
  cargo install sqlx-cli --no-default-features --features rustls,mysql
  ```

## Configuration

Copy `.env.example` to `.env` and fill it in:

| Var | Meaning |
|---|---|
| `DATABASE_URL` | `mysql://user:pass@host:port/electronix_id` |
| `BIND_ADDR` | api listen address, e.g. `0.0.0.0:8080` |
| `RESOLVER_BIND_ADDR` | resolver listen address (default `0.0.0.0:8081`) |
| `JWT_SECRET` | HS256 secret, **≥ 32 bytes** (startup fails otherwise) |
| `ACCESS_TOKEN_TTL_SECS` | access-token lifetime (default 900) |
| `REFRESH_TOKEN_TTL_SECS` | refresh-token lifetime (default 2592000) |
| `STORAGE_ROOT` | local file-storage root (default `./storage`) |
| `MAX_UPLOAD_BYTES` | per-upload size cap (default 52428800) |
| `RUST_LOG` | tracing filter |
| `CORS_ALLOWED_ORIGINS` | comma-separated allowed origins |

## Database & run

```bash
sqlx database create
sqlx migrate run                 # api owns the schema
cargo run -p electronix-id-api       # tenant API on BIND_ADDR; GET /health -> 200
cargo run -p electronix-id-resolver  # public resolver on RESOLVER_BIND_ADDR
```

## Quality gates

```bash
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
cargo sqlx prepare --workspace   # regenerate ./.sqlx before committing
```

`.env` and `/storage` are git-ignored; `./.sqlx` is committed so the project
builds without a live DB.

## Endpoints

All under `/api/v1` (JSON, `snake_case`) except health. Protected routes take a
`Bearer` access token. List endpoints return `{ data, page, per_page, total }`;
errors return `{ error: { code, message } }`; money is `{ amount_minor, currency }`.

**Health**
- `GET /health` · `GET /health/ready` (DB `SELECT 1`)

**Auth**
- `POST /auth/register` · `POST /auth/login` · `POST /auth/refresh` (rotates)
  · `POST /auth/logout` · `GET /auth/me`

**Users** (read = any member; write = admin/owner)
- `GET /users` · `POST /users` · `GET /users/{id}` · `PATCH /users/{id}` · `DELETE /users/{id}`

**Organization & billing**
- `GET /organization` · `PATCH /organization`
- `GET /organization/subscription` · `GET /organization/billing/estimate`

**Machines** (org-scoped, paginated)
- `GET /machines` · `POST /machines` · `GET /machines/{id}` · `PATCH /machines/{id}`
  · `DELETE /machines/{id}` · `PATCH /machines/{id}/tier`
- `POST /machines/{id}/tag/rotate` (admin/owner) — issue a fresh public scan code,
  revoking the old QR tag. Each machine is born with a `public_code`.

**Documents & versions** (generic, versioned)
- `GET /machines/{machine_id}/documents` (filter `?category=`) · `POST /machines/{machine_id}/documents`
- `GET /documents/{document_id}` · `PATCH /documents/{document_id}` · `DELETE /documents/{document_id}`
- `POST /documents/{document_id}/versions` (multipart for `file`, JSON body for `json`)
- `GET /documents/{document_id}/versions` · `GET /documents/{document_id}/versions/{version_no}`
- `GET /documents/{document_id}/versions/{version_no}/download` (streams bytes)
- `POST /documents/{document_id}/versions/{version_no}/restore`

**Pricing**
- `GET /plans`

### Resolver service (`:8081`, separate binary)

Public QR/scan API. A machine's QR encodes its `public_code`; the resolver maps
it to a passport. JSON, `snake_case`, same error envelope as the api.

- `GET /health` · `GET /health/ready`
- `GET /r/{code}` — **public** passport summary (name, make/model, status, org,
  photo link). No documents, no internal ids.
- `GET /r/{code}/photo` — **public** primary-photo stream.
- `GET /r/{code}/full` — **gated**: requires a `Bearer` token whose org owns the
  machine (cross-org → 404). Returns the document inventory with current versions.
