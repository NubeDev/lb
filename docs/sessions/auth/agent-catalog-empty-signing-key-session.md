# Session — Agent catalog empty though backend seeds 6 (signing-key persistence)

- **Date:** 2026-07-03
- **Area:** auth / gateway (frontend symptom)
- **Status:** shipped, green

## The ask

Settings → Agent renders "No agent definitions available for this node." even though the node
boots with `seeded 6 agent definitions` and `agent.def.list` / `GET /agent/defs` return 6 when
curled against `:8080`. Find why the browser shows empty while the backend returns 6, and fix the
root cause so the seeded agents render. (Backend seeding was pre-confirmed — out of scope to
re-verify.)

## What I did

1. Traced the UI catalog path: `useAgentCatalog` → `listAgentDefs()` →
   `invoke("mcp_call", {tool:"agent.def.list"})` → `POST /mcp/call`. The UI uses the **MCP bridge**,
   not the `/agent/defs` REST route the working curl hit.
2. Booted the persistent-store node (`target/debug/node`, `LB_STORE_PATH=.lazybones/data/dev-store`)
   and reproduced the request path with curl:
   - `POST /mcp/call {tool:"agent.def.list"}` + a **fresh** login token → 6 defs, shape
     `{definitions:[…]}` (server bridge fine).
   - CORS preflight + POST from origin `:5173` → `access-control-allow-origin: *` (not CORS).
   - `POST /mcp/call` + a **bogus** token → **401**.
3. Found the swallow: `useAgentCatalog` maps any rejected load to `[]` via `Promise.allSettled`,
   and `AgentCatalog.tsx` renders the same empty-state text for `[]` whether empty or failed.
4. Found the root cause: `Gateway::boot()` used `SigningKey::generate()` **per process** — a fresh
   random signing key each start — while the store is durable. A node restart invalidates every
   previously-issued (localStorage-rehydrated) browser token → 401 → empty catalog.

## The fix

- **Backend (root cause):** `rust/role/gateway/src/signing_key.rs` — persist the signing seed at
  `<store>.signing-seed` beside a durable store and reload it (`from_seed`) on the next boot;
  ephemeral `generate()` only when there's no `LB_STORE_PATH`. `Gateway::boot()` now calls
  `signing_key::resolve()`. Tokens survive a restart.
- **Frontend (symptom):** `ui/src/lib/ipc/http.ts` — an authenticated `401` clears the session so
  the app falls back to login instead of a logged-in shell full of silent-empty panels; a `403`
  (capability Denied) is left alone.

## Tests (green)

- `rust/role/gateway/src/signing_key.rs`:
  `seed_persists_so_a_pre_restart_token_verifies_after_restart`,
  `a_different_store_gets_a_different_key` — both pass. Whole `lb-role-gateway` suite green.
- `ui/src/lib/ipc/http.session-expiry.test.ts`: 401 clears session / 403 does not — both pass.
  Full UI unit suite: **424 passed (66 files)**.

## Why the two-boot proof is a unit test, not a manual restart

This environment reaps backgrounded server processes when a tool call returns, so a hand-run
"boot → login → restart → re-check the old token" dance couldn't be held across steps. The
`signing_key` unit test reproduces exactly that scenario deterministically (mint with boot-1's key,
verify with boot-2's key resolved from the same store path) — a stronger, repeatable proof.

## Notes / follow-ups

- Dev-grade key custody (a seed file beside the store). README §13's **deployment** key custody
  (keychain/secret store, rotation across roles) is still the open question — unchanged by this.
- The API-key pepper (`state.rs`) has the same "random per process, doesn't survive a restart"
  limitation by the same reasoning; not addressed here (API keys weren't the reported symptom), but
  a candidate for the same seed-beside-the-store treatment if dev API keys need to survive restarts.

## Files touched

- `rust/role/gateway/src/signing_key.rs` (new) + `lib.rs` (mod) + `state.rs` (`boot()` wiring)
- `ui/src/lib/ipc/http.ts` (401 → clear session) + `http.session-expiry.test.ts` (new)
- `docs/debugging/auth/signing-key-not-persisted-invalidates-sessions.md` (new) + README row
