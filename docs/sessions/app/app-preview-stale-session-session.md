# app — preview stale-session trap: validated restore (session)

- Date: 2026-07-04
- Scope: follow-on to [app-preview-login-session.md](app-preview-login-session.md) (which made the
  preview gateway in-memory on port 8087) — closes the UX trap that in-memory-ness introduced
- Stage: post-S10 app slice 1 of 3 (shell), preview hardening
- Status: **done** — a restart-invalidated session now falls to login instead of showing an empty
  channel list; 17/17 gateway tests green, Playwright e2e green

## What this session set out to do

The prior session made the preview gateway a **throwaway in-memory `test_gateway` on 8087**, which
fixed the port-collision 403 — but introduced a new UX trap: the browser persists its session token
in `localStorage`, and every `make -C app dev` restart wipes the in-memory store AND mints a fresh
signing key. So after a restart the persisted token is dead, yet the shell rehydrated it blindly and
rendered **"No channels yet — create one below."** The user read that as "the channels I created
disappeared." Task: make it impossible to confuse the user this way, per CLAUDE.md rules (esp.
rule 9 — real gateway, no fakes; keep the native path untouched).

## Root cause

`session.restore()` ([session.store.ts](../../../app/sdk/src/session/session.store.ts)) only reads
the persisted token *back* — it never contacts the node — so a dead-but-well-formed token drove a
logged-in UI. The 401→drop path
(`request.ts`→`config.onAuthError`→`session.logout(ws)`) existed but was only reached
opportunistically by whichever later request happened to 401, and **not at all** during the restart
window when the node is briefly down (`fetch` rejects with a network error, not a 401).

This is the **in-memory-preview twin** of the durable-node bug fixed in
[auth/signing-key-not-persisted-invalidates-sessions.md](../../debugging/auth/signing-key-not-persisted-invalidates-sessions.md).
There the node had a durable store, so the fix was to persist the signing key beside it (tokens
survive a restart). Here the preview node is **deliberately ephemeral** — persisting its key would
just make it honour a token for channels that no longer exist. So the fix belongs on the **client**:
prove the rehydrated token is live before trusting it.

## The fix (SDK-layer validated restore)

Weighed the three handover options. Chose **validate-on-restore**, keeping the gateway in-memory:

- **New `app/sdk/src/session/validate.ts` → `probeSession(config)`**: one cheap authenticated read
  (`GET /workspaces` — every member holds `workspace.list`, and the shell reads it on boot anyway).
  Classifies the *outcome*, not the payload: `401` → `"dead"`; `fetch` rejects → `"unreachable"`;
  anything else (403/2xx) → `"live"` (the node authenticated the token — a cap deny is not staleness).
- **New `client.restore()` in [create.ts](../../../app/sdk/src/client/create.ts)**: rehydrates via
  the store, then probes. Drops the session on `"dead"`, or on `"unreachable"` unless the caller
  passed `onUnreachable: "keep"`. The drop calls `session.logout(ws)`, which notifies subscribers →
  `useSession` re-renders → `App.tsx` shows `LoginScreen`.
- **`GatewayClientOptions.onUnreachable: "drop" | "keep"`**: the preview passes `"drop"` (a
  throwaway node makes an unverifiable session worthless — one prefilled click to re-login). A device
  build against a durable node can pass `"keep"` to stay logged in offline. A `"dead"` (401) session
  is always dropped regardless.
- **[app/shell/src/lib/client.ts](../../../app/shell/src/lib/client.ts)** now calls
  `client.restore()` with `onUnreachable: "drop"` in place of the raw `session.restore()`.

Why the SDK and not the shell: the native build calls the same `restore()`, so both platforms get
the guarantee with one implementation, and there is **no core/gateway change** — the gateway already
401s a dead token correctly. No branching on any extension id; the probe is a generic authenticated
read.

### Why NOT make the preview store durable (handover option 2)

Tempting, because `signing_key.rs` persists the seed *beside* `LB_STORE_PATH`, so one env var would
make both channels and tokens survive a restart. Rejected as the default: a durable preview store
re-introduces the exact **"first login owns the workspace"** bootstrap problem the prior session
escaped by going in-memory (a second dev/user then 403s "not a member"), and ephemerality is a
*feature* for a throwaway preview. The free re-login costs one click with prefilled credentials.
`onUnreachable: "keep"` is the knob left for a durable-node consumer; a `make -C app gateway CLEAN=1`
would be the escape hatch if durability is ever wanted — not added now (no consumer).

## Testing (rule 9 — real spawned gateway, no fakes)

- **`app/sdk/tests/restore-liveness.gateway.test.ts`** (new, 4 cases, all vs the REAL `test_gateway`):
  - live-kept: a REAL login token round-trips through `restore()` and survives.
  - dead-401-dropped: a shaped-but-unsigned token hits the gateway's REAL verify path, is 401'd,
    and the session is dropped (subscribers notified → login).
  - unreachable-dropped: the client pointed at a closed port (real network failure) with
    `onUnreachable:"drop"` → dropped.
  - unreachable-kept: same, `onUnreachable:"keep"` → session retained (device offline policy).
  - `cd app/sdk && pnpm test:gateway` → **17/17** (13 prior + 4 new).
- **Playwright e2e** (scratchpad `run-e2e.sh` + `e2e-restart.mjs`, cached puppeteer Chrome):
  login ada/acme → create `room-before-restart` (visible) → **kill + reboot** the gateway on 8087
  (fresh in-memory store + fresh key = dead token) → reload → asserted the shell shows
  **"Lazybones / Sign in"**, NOT the stale/empty room, and does not ghost the vanished channel.
  Console shows the real 401 the fix now catches. Screenshot: the clean prefilled login screen.
- `make -C app typecheck` → clean (after `pnpm install --ignore-workspace` in `app/shell` to refresh
  the `file:`-copied `@nube/app-sdk` — the shell's copy is a snapshot, not a live symlink; an SDK
  edit needs a reinstall to propagate to the shell typecheck/build).

## Files touched

- `app/sdk/src/session/validate.ts` (new) — `probeSession` + `SessionLiveness`.
- `app/sdk/src/client/create.ts` — `client.restore()`, `onUnreachable` option, wiring.
- `app/sdk/src/index.ts` — export `probeSession` / `SessionLiveness`.
- `app/shell/src/lib/client.ts` — call `client.restore({onUnreachable:"drop"})` on boot.
- `app/sdk/tests/restore-liveness.gateway.test.ts` (new) — the regression.
- Docs: this session, `docs/debugging/app/stale-preview-session-shows-empty.md` (+ README row),
  `docs/public/app/app.md`, `docs/STATUS.md`, memory `app-preview-port-and-prefill.md`.

## Gotcha for the next session

The shell consumes `@nube/app-sdk` as a pnpm `file:` dep that is **copied, not symlinked**. After
editing SDK source, `cd app/shell && pnpm install --ignore-workspace` to refresh the copy or the
shell typecheck/build sees the OLD SDK (`Property 'restore' does not exist` etc.). The gateway tests
(`app/sdk`) import `../src` directly and don't need this.
