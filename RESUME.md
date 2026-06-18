# RESUME — ElectronIx ID backend

Session handoff. Spec = `../CLAUDE.md`. **This milestone (M0–M8) is COMPLETE.**
Next session starts the *next* milestone (resolver + QR scan — see bottom).

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
| `.sqlx` builds without a live DB | ✅ generated (44 queries) — see caveat |
| `migrations/` full schema + 3 plans seeded | ✅ |
| README documents env vars + run/test | ✅ (endpoint list now full too) |
| Layer boundaries respected | ✅ no sqlx outside `infrastructure`; no logic in `web`; pure `domain` |

**⚠ One open item:** the repo is **not a git repo** (`is git: false`), so `.sqlx`
is generated but *not committed*. To satisfy "commit `.sqlx`": `git init`, add a
remote if wanted, then commit (`.gitignore` already excludes `target/ .env /storage`
and keeps `.sqlx`).

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

## NEXT MILESTONE (start here next session)
Per `CLAUDE.md` §14 final line + §2 "out of scope (future crates)": the next,
**separate** milestone adds the **`resolver` crate and the QR scan path**.

Likely shape (confirm scope with user before building):
- New workspace member `crates/resolver` (add to root `Cargo.toml` members).
- QR/tag → machine resolution endpoint(s): scan a tag id → public-ish machine
  passport view (current photo + current document versions), respecting tiers.
- Decide auth model for scans (public read vs. token-gated) and what a scan
  exposes vs. the authenticated API.
- Keep the same layering; reuse `domain` (extract a `shared` crate only when the
  edge agent arrives — not yet, per §4).
- Still out of scope: MQTT, edge-agent, ingest, taggen, live telemetry, payments,
  R2 wiring, Next.js UI.

Other backlog (optional, not blocking): `utoipa` OpenAPI (§8 optional); `git init`
to actually commit `.sqlx`.
