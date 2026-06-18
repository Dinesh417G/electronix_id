# CLAUDE.md — Bruno API collection (keep in sync with the backend)

This folder is the **Bruno collection** that tests the ElectronIx ID backend. It is a
maintained artifact, not a throwaway. When you change the API, update this collection in the
**same change** so it never drifts from reality.

## What this folder is
- `bruno.json` — collection manifest (do not rename the collection casually; it's referenced by docs).
- `collection.bru` — collection-level **bearer auth** using `{{accessToken}}`. Auth requests set `auth: none`; everything else uses `auth: inherit`.
- `environments/Local.bru` — `rootUrl` (host root, for `/health`), `baseUrl` (`…/api/v1`), `resolverUrl` (the resolver binary on :8081), and a shared `userPassword`. Add a `Staging.bru` here when staging exists; never put real secrets in committed env files.
- One folder per resource: `Health`, `Auth`, `Machines`, `Documents`, `Pricing`, `Users`, `Organization`, `Resolver`, plus `Cleanup` (runs last). `folder.bru` `seq` controls run order. The `Resolver` folder targets a **separate binary** on :8081 (`electronix-id-resolver`) — it must be running or that folder errors on connect.
- `samples/` — fixture files used by multipart uploads.

## Rules when you touch the API
1. **New endpoint → new `.bru` request** in the matching folder, with the next `seq`. Include an `assert` block on `res.status` (and key fields), and a short `docs` block.
2. **Changed request/response shape** → update the affected `body:json`, `params:query`, `vars:post-response`, and `assert` blocks. Response field renames must be reflected in every `vars:post-response` that reads them.
3. **New id returned by a create endpoint** → capture it with `vars:post-response` (e.g. `thingId: res.body.id`) so later requests can reference `{{thingId}}`.
4. **Destructive requests** (DELETE) go in the `Cleanup` folder so a full run doesn't break dependent requests.
5. **Keep the variable chain intact:** Register/Login set `accessToken` + `refreshToken`; Refresh stashes `prevRefreshToken` (consumed by `Refresh Reuse Rejected`); Create Machine sets `machineId` + `publicCode`; Rotate Tag re-sets `publicCode`; Create Document Slot sets `documentId`; Create JSON Spec Slot sets `jsonDocId`; Create User sets `userId`. Don't break these without updating downstream requests.
6. **JSON stays `snake_case`** to match the API contract.

## Definition of done for an API change
- The collection runs clean against a local server (`Auth → … → Cleanup`) with all asserts passing.
- Every endpoint in the OpenAPI/route list has at least one request here.
- No request references a variable that nothing sets.

## How to run (for reference)
Open the folder in Bruno (desktop), select the **Local** environment, then either click requests
top-to-bottom or use **Run Collection**. CLI: `bru run --env Local` from this folder.
