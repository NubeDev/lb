# Agent catalog shows "No agent definitions available" though the backend seeds 6

- **Area:** auth (surfacing as a frontend symptom)
- **Status:** resolved
- **Date:** 2026-07-03

## Symptom

Settings → Agent showed **"No agent definitions available for this node."** even though boot
logged `boot: seeded 6 agent definitions` and both `GET /agent/defs` and the `agent.def.list`
MCP verb returned `count: 6` when curled directly against `:8080` with a **freshly minted**
token. It reproduced with the node confirmed up. The prior theory ("node was down when the page
loaded") was wrong.

## Investigation

The UI catalog reads over the MCP bridge (`ui/src/lib/agent/agentDef.api.ts` →
`invoke("mcp_call", { tool: "agent.def.list" })` → `POST /mcp/call`), NOT the `/agent/defs`
REST route. So the working curl and the browser exercised **different transports**.

Ruled out, in order, against the live node:

1. **Server dispatch** — curling `POST /mcp/call {tool:"agent.def.list"}` with a **fresh** login
   token returned all 6 with the exact `{definitions:[…]}` shape `listAgentDefs()` expects. The
   bridge path is fine.
2. **CORS** — the preflight `OPTIONS /mcp/call` from origin `:5173` returned
   `access-control-allow-origin: *` / `-methods: *` / `-headers: *`, and the real POST echoed the
   CORS headers. Not CORS.
3. **Client state** — a `POST /mcp/call` with a **bogus** bearer returned **401**. And
   `useAgentCatalog` wraps the three loads in `Promise.allSettled(...).then(... defs.status ===
   "fulfilled" ? defs.value : [])` — so **any** rejection of `listAgentDefs()` (a 401 included)
   silently becomes `definitions = []`, which `AgentCatalog.tsx` renders with the identical
   "No agent definitions available" text it uses for a genuinely empty catalog. The empty state
   and an auth failure were indistinguishable.

## Root cause

`Gateway::boot()` (`rust/role/gateway/src/state.rs`) minted the node's token-signing key with
`SigningKey::generate()` **every process start** — a fresh random key. The SurrealKV store is
durable (`LB_STORE_PATH`, e.g. `.lazybones/data/dev-store`), but the signing key was **not**
paired with it. So across a restart of the persistent dev node:

1. Boot A (key A): the browser logs in → token signed with key A → saved to `localStorage`.
2. Node restarts → boot B (key B, random). The store still holds the 6 seeded defs.
3. The browser rehydrates the **key-A** token from `localStorage` (`session.store.ts` loads it on
   startup and keeps using it — the gateway is meant to re-check server-side) → every request →
   `verify` against **key B** → **401** → `listAgentDefs()` rejects → empty catalog.

The persistent store made it look continuous while the signing identity silently rotated
underneath — exactly "up, returns 6 via a fresh curl, but the browser's stored session is dead."
The API-key pepper had the identical documented limitation ("do not survive a restart, like the
dev-login").

## Fix

Two layers:

1. **Root cause (backend).** New `rust/role/gateway/src/signing_key.rs`: `resolve()` persists the
   32-byte signing seed at `<store>.signing-seed` beside a durable store (`LB_STORE_PATH` set) and
   reloads it on the next boot via `SigningKey::from_seed`; with no store path (in-memory
   dev/test) it keeps the ephemeral `generate()`. The seed file is written `0600`; any IO/format
   error degrades to an ephemeral key rather than failing boot. `Gateway::boot()` now calls
   `crate::signing_key::resolve()` instead of `SigningKey::generate()`. Tokens now survive a
   restart against the same store — the persistent dev loop behaves like a persistent store. (Dev
   custody; README §13's deployment key custody is still open.)

2. **Symptom robustness (frontend).** `ui/src/lib/ipc/http.ts`: a `401` on a request we **did**
   authenticate clears the session (`setSession(null)`) so the app falls back to the login screen
   instead of a logged-in shell whose every read silently rejected to an empty state. A `403`
   (capability Denied) is left alone — the caller is still authenticated, just lacks the cap.

## Regression tests

- `rust/role/gateway/src/signing_key.rs` unit tests:
  `seed_persists_so_a_pre_restart_token_verifies_after_restart` (a token minted with boot-1's
  resolved key verifies against boot-2's key resolved from the **same** store path — the exact
  restart-survival invariant) and `a_different_store_gets_a_different_key` (tokens don't cross
  stores).
- `ui/src/lib/ipc/http.session-expiry.test.ts`: an authenticated `401` clears the session; a
  `403` does not.

## Lessons

- **Persist the signing key with the store, not the process.** A durable store paired with an
  ephemeral signing key silently invalidates every rehydrated session on restart, and the failure
  is invisible: a fresh curl works, the browser's stored token 401s.
- **A swallowed rejection must not render as "empty."** `Promise.allSettled(...) ? value : []`
  collapses auth/network failure into the genuinely-empty UI state. Distinguish "the fetch failed"
  from "there is nothing" — here, a 401 should log out, not render an empty catalog.
