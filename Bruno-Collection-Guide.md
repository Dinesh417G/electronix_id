# Using the ElectronIx ID Bruno Collection — Step by Step

Bruno is a desktop API client. This collection (`electronix-id-api/`) is a ready-made set of
requests for your backend, with auth tokens captured automatically and assertions on every call,
so it doubles as a click-through smoke test.

> Think of it as the HMI for your API: saved, labelled buttons instead of typed terminal commands.

---

## 1. Install Bruno

- Download from **usebruno.com** (Windows / macOS / Linux), or on Windows: `winget install Bruno.Bruno`.
- It's a normal desktop app, free, no account, works offline. Your data stays on your machine.

## 2. Get the collection into place

1. Unzip `electronix-id-api-bruno.zip`.
2. Put the `electronix-id-api/` folder wherever you like — ideally **inside your backend repo** (e.g. `repo/api-tests/electronix-id-api/`) so it's versioned with the code.

## 3. Open it in Bruno

1. Launch Bruno → **Open Collection** → select the `electronix-id-api` folder (the one containing `bruno.json`).
2. The left sidebar now shows the folders: Auth, Machines, Documents, Pricing, Users, Organization, Cleanup.

## 4. Select the environment (do this first — nothing works without it)

1. Top-right environment dropdown → choose **Local**.
2. Click the environment (the small settings/gear next to it) to see its variables:
   - `rootUrl` = `http://localhost:8080` — host root, used by the Health probes (`/health` is **not** under `/api/v1`).
   - `baseUrl` = `http://localhost:8080/api/v1` — change this if your server runs on a different host/port (e.g. your MySQL-style `:3308` story doesn't apply here; this is the API port from `BIND_ADDR`).
   - `resolverUrl` = `http://localhost:8081` — the separate public resolver binary (`electronix-id-resolver`). Only the Resolver folder uses it.
   - `userPassword` = the test password used by Register/Login.
3. Save if you changed anything.

## 5. Start your backend

In the repo: `cargo run` (or the release binary). Confirm it's up — run **Health → Liveness**... (this collection assumes the API is reachable at `baseUrl`).

## 6. Run the happy path, in order

Click each request, hit **Send**, read the response on the right. Go top to bottom:

1. **Auth → Register** — creates an org + owner and **captures the access & refresh tokens automatically**. Watch the assertions panel go green (status 201). You don't copy any token by hand — that's the point.
2. **Auth → Me** — returns your user + org. It silently reuses the captured token (collection-level bearer auth). Confirm there's no password hash anywhere in the response.
3. **Auth → Refresh** — get new tokens; the old refresh token is now revoked. The next request, **Refresh Reuse Rejected**, replays that consumed token and asserts **401** — rotation proof, automated (no manual var editing).
4. **Machines → Create Machine** — captures `machineId` **and** `publicCode`. **List / Get / Update / Change Tier** operate on it. (`Create Machine` does **not** take a plan — set the tier with **Change Tier**.) **Rotate Tag** issues a fresh `publicCode` and revokes the old QR.
5. **Documents → Create Document Slot** — captures `documentId`. **Upload Version 1**, then **Upload Version 2** (multipart; they post the file in `samples/`). **List Versions** should show 2 with exactly one `is_current`. **Download Version 1** should return the sample file's bytes. **Restore Version 1** creates a new version copying v1. Then **Create JSON Spec Slot** (captures `jsonDocId`) + **Upload JSON Version** exercise the json-kind path — note the payload field is **`content`**, not `content_json`.
6. **Pricing → List Plans / Subscription / Billing Estimate** — estimate should equal the sum of your active machines' plan prices.
7. **Users → Create User / List / Get / Update** — captures `userId`.
8. **Organization → Get / Update**.
9. **Resolver → Resolve Summary / Full / Full Unauthorized / Photo** — the public QR path on the **separate :8081 binary**. Start `cargo run -p electronix-id-resolver` first, or skip this folder. Uses `publicCode`.
10. **Cleanup → Delete User / Delete Machine** — run these **last**; deleting the machine cascades to its documents.

## 7. Watch variables get captured (so you understand the magic)

- Top bar → **Variables** (or the `{x}` icon) shows runtime variables: after Register you'll see `accessToken`, `refreshToken`; after Create Machine, `machineId` + `publicCode`; after Create Document Slot, `documentId`; after Create JSON Spec Slot, `jsonDocId`.
- Any request using `{{machineId}}` in its URL is resolving one of these. If a request 404s with an empty-looking id, it means the request that *sets* that variable hasn't been run yet this session.

## 8. Run the whole thing at once (the smoke test)

- Right-click the collection name → **Run Collection** (Bruno's runner). It executes every request in `seq` order and shows pass/fail per assertion.
- Because Register uses a randomized email each run, you can re-run it repeatedly without "email already taken".
- Green across the board = your happy path is intact. Run this after every backend change as a quick regression pass.

## 9. CLI (optional, for CI later)

From inside the collection folder:
```bash
npm install -g @usebruno/cli
bru run --env Local
```
This is what you'd wire into CI to smoke-test a deployed staging server (point `baseUrl` at staging via a `Staging` environment).

---

## 10. Use it to actually break things (beyond the happy path)

The collection proves the happy path. Production safety needs the adversarial checks from the
verification guide — easy to do here by editing variables:

- **Tenant isolation.** Run Register a second time (a second org B with its own token). Then, in the Variables panel, keep org B's `accessToken` but manually set `machineId` to org **A's** machine id, and Send **Get Machine**. It must return **404/403**, never org A's data. Repeat for documents.
- **RBAC.** Create a `viewer` user (set `role` to `viewer` in Create User), log in as them (set `runEmail`/password and run Login), then try **Create Machine** or **Change Tier** → must be **403**.
- **Bad input.** Edit a `body:json` to send an empty `name` or malformed email → expect **422** with a field error, never a 500.
- **Auth edge cases.** Blank the `accessToken` variable and call **Me** → **401**. Tamper one character of the token → **401**.

---

## Keeping the collection alive (add to your root CLAUDE.md)

A test collection that drifts from the API is worse than none. There's a `CLAUDE.md` inside the
collection folder telling Claude Code to update the `.bru` files whenever it changes an endpoint.
If you'd rather keep one instruction file, paste this line into your **root** `CLAUDE.md` instead:

> When you add or change an API endpoint, update the Bruno collection under `api-tests/electronix-id-api/` in the same change: add/modify the matching `.bru` request, keep the `vars:post-response` capture chain intact (`accessToken`, `machineId`, `documentId`, `userId`), assert on `res.status`, and ensure a full `Run Collection` passes against a local server. JSON stays `snake_case`.

That one rule keeps your API and its tests in lockstep as the backend grows.
