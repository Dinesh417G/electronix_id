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
├── Cargo.toml          # workspace, member crates/api
├── migrations/         # sqlx migrations (0001_init.sql, ...)
└── crates/api/         # the API binary; layers are modules under src/
    └── src/{domain, application, infrastructure, web}
```

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
| `BIND_ADDR` | listen address, e.g. `0.0.0.0:8080` |
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
sqlx migrate run
cargo run            # serves on BIND_ADDR; GET /health -> 200
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

**Documents & versions** (generic, versioned)
- `GET /machines/{machine_id}/documents` (filter `?category=`) · `POST /machines/{machine_id}/documents`
- `GET /documents/{document_id}` · `PATCH /documents/{document_id}` · `DELETE /documents/{document_id}`
- `POST /documents/{document_id}/versions` (multipart for `file`, JSON body for `json`)
- `GET /documents/{document_id}/versions` · `GET /documents/{document_id}/versions/{version_no}`
- `GET /documents/{document_id}/versions/{version_no}/download` (streams bytes)
- `POST /documents/{document_id}/versions/{version_no}/restore`

**Pricing**
- `GET /plans`
