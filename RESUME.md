# RESUME — ElectronIx ID backend

Session handoff. Spec = `../CLAUDE.md`. **M0–M8 COMPLETE.**
**Resolver/QR-scan milestone also COMPLETE** (see "Resolver milestone" below).
Next session: see "NEXT MILESTONE" at the bottom.

---

## How to run / verify (do this first next session)

```bash
# .env already exists at workspace root, pointing at the live DB:
#   DATABASE_URL=mysql://root:Test12345@127.0.0.1:3308/electronix_id
export DATABASE_URL='mysql://root:Test12345@127.0.0.1:3308/electronix_id'

cargo test                                    # 40 passing
cargo clippy --all-targets -- -D warnings     # clean
cargo fmt --check                             # clean
cargo run                                     # serves on 0.0.0.0:8080
```

- sqlx-cli installed = **0.9.0** (crate pins sqlx 0.8). `cargo sqlx prepare --workspace`
  worked fine and wrote `.sqlx/` (44 queries) at the workspace root.
- Offline build verified: `unset DATABASE_URL; SQLX_OFFLINE=true cargo check --all-targets`.

---

## DoD §14 cross-check

| DoD item | Status |
|---|---|
| M0–M8 complete, ACs met | ✅ |
| `cargo fmt --check` clean | ✅ |
| `cargo clippy --all-targets -- -D warnings` clean | ✅ |
| `cargo test` green | ✅ 40 tests (6 suites) |
| `.sqlx` builds without a live DB | ✅ generated (44 queries) + committed |
| `migrations/` full schema + 3 plans seeded | ✅ |
| README documents env vars + run/test | ✅ (endpoint list now full too) |
| Layer boundaries respected | ✅ no sqlx outside `infrastructure`; no logic in `web`; pure `domain` |

**Git:** initialized + pushed. Remote `origin` = `https://github.com/Dinesh417G/electronix_id.git`,
branch `main`. Initial commit = 118 files (`.sqlx` committed). `.gitignore` excludes
`target/ .idea/ .env /storage`, keeps `.sqlx`.

---

## Test coverage (40 tests, 6 suites)
- **domain** unit tests (value objects, slug, version monotonic, money, plan flags).
- **services** (11) — in-memory fakes (`tests/common`): auth flow, RBAC, cross-org,
  version-bump + restore, pricing math.
- **repositories** (6) — `#[sqlx::test]`, real MySQL: org/user/plan/subscription/
  machine CRUD + org-scoping + the §5 document version transaction + photo→primary.
- **handlers** (5) — `tower::oneshot`, real MySQL: auth lifecycle, cross-org
  isolation (org A token → org B machine = 404), RBAC viewer-forbidden, pricing
  estimate/catalog/subscription, JSON document version + restore over HTTP.

---

## Built this session (M2-finish → M8)
- `infrastructure/persistence/` — MySQL impls of all 7 repos (organization, user,
  machine, plan, subscription, refresh_token, document). `query!`/`query_as!`.
  §5 version-bump txn in `mysql_document_repo::add_version`.
- `state.rs::AppState::new(pool, settings)` composition root; `lib.rs run()` uses it.
  Added `AuthService::authenticate`, `PricingService::{plans, subscription}`.
- `web/` — `dto.rs`, `pagination.rs` (`Page<T>`, `Data<T>`), `extractors.rs`
  (`AuthUser`, `ValidatedJson<T>`), `middleware.rs` (CORS), all `handlers/*`,
  `router.rs` (all `/api/v1` routes + request-id + trace + CORS + body-limit).
- `error.rs` — `AppError: From<ApplicationError> + From<DomainError>`.

---

## DB notes
- DB name = **`electronix_id`** (created fresh on 3308). Existing `electronix_factory`
  left untouched (decided with user).
- Live smoke test left one org row "Smoke Test Co" in `electronix_id` — harmless
  dev data; `DELETE` it for a pristine DB if desired.

## Deviations (report carried forward)
1. Added `subscription_repo` + `refresh_token_repo` ports (clean separation).
2. `UserRepository::find_by_id_any` — unscoped, auth-internal (refresh path).
3. File storage key = version **UUID**, not sequential `version_no`.
4. §5 atomic SQL lives in `DocumentRepository::add_version`; service orchestrates.
5. JSON columns read via `CAST(col AS CHAR)` → raw text in the domain.
6. Document/version list endpoints use `{data:[...]}` (not the paginated envelope).
7. `POST /machines/{id}/documents` creates the slot only (first version is a
   separate `POST .../versions`).
8. Machine PATCH on nullable fields sets the given value; cannot clear-to-null
   (no double-option in the DTO).

---

## Resolver milestone (COMPLETE)

New workspace member **`crates/resolver`** — a separate **public** binary (port
`:8081`, `RESOLVER_BIND_ADDR`) that maps a machine's opaque QR tag code to a
passport view. Reuses the api crate as a library (domain, repo traits + MySQL
adapters, JWT, storage, `Settings`); owns **no** persistence code and runs **no**
migrations (api owns the schema). All `query!` macros stay in api, so the single
`./.sqlx` cache covers both crates.

**Scope decisions (made with user via AskUserQuestion):**
1. Deploy shape = **separate binary service** (not routes in api, not a lib-only crate).
2. Scan auth = **public minimal summary + login-gated full detail**.
3. Tag scheme = **opaque per-machine `public_code`** (rotatable), not the UUID.

**api-side changes (tag scheme lives on the machine):**
- `migrations/0003_machine_public_code.sql` — `machines.public_code CHAR(16) NULL
  UNIQUE`; existing rows backfilled from the id hex, new rows get a random
  Crockford-base32 code (`value_objects::PublicCode::generate`, 16 chars ≈ 2^80).
- `Machine.public_code` field; repo SELECT/INSERT/UPDATE carry it; new **unscoped**
  `MachineRepository::find_by_public_code` + `DocumentRepository::find_version_by_id`
  (for the public photo via `primary_photo_version_id`).
- `MachineService`: generates a code on create; `rotate_public_code` (admin/owner).
- `MachineResponse.public_code`; `POST /api/v1/machines/{id}/tag/rotate`.

**resolver crate** (`src/{config,state,auth,dto,handlers,router,lib}.rs` + bin):
- `GET /r/{code}` public summary · `GET /r/{code}/photo` public photo stream ·
  `GET /r/{code}/full` gated (token whose org owns the machine; cross-org → 404).
- Reuses api's `AppError` for an identical error envelope. `ScanViewer` extractor
  verifies the bearer JWT via the shared secret.
- Tests (`tests/resolve.rs`, `#[sqlx::test]`): seed via the **api** router, exercise
  the **resolver** router over the shared pool — summary fields, unknown→404,
  full requires auth (401), cross-org→404, owner sees inventory, photo-absent→404,
  and tag-rotation revokes the old code.

*Verify:* `cargo run -p electronix-id-resolver`. Gates all green: `cargo fmt
--check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` = **43 tests**.
`./.sqlx` regenerated (`cargo sqlx prepare --workspace`) and committed.

---

## NEXT MILESTONE (start here next session)
Per `CLAUDE.md` §2 "out of scope (future crates)". Confirm scope with the user
first. Candidates, roughly in order:
- **`edge-agent` / `ingest`** + MQTT and the LIVE-tier telemetry data path
  (`tier_allows(machine, "live_data")` is already wired as the gate).
- **`taggen`** — generate the printable QR assets that encode `/r/{public_code}`.
- When a *second* consumer of the domain lands (the edge agent), extract a
  `shared` crate (per §4) instead of depending on the whole `api` crate as we do
  for the resolver now.

Still out of scope: payments/Razorpay, Cloudflare R2 wiring (behind `FileStorage`),
Next.js UI, Tauri field app.

Other backlog (optional, not blocking): `utoipa` OpenAPI (§8 optional).
