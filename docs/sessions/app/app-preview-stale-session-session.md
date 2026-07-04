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

## Second bug, found on live-verify: bare login handle ≠ member (the actual "can't login")

After the restore fix, the user still couldn't log in — the screenshot showed **"Failed to fetch"**
on Sign in. Driving the real path exposed a **separate, deeper** bug (full card:
[bare-login-handle-not-a-member.md](../../debugging/app/bare-login-handle-not-a-member.md)):

- **"Failed to fetch"** = the preview defaulted `?node=` to **8087** (the app's own `test_gateway`),
  but the user runs root `make dev`, whose node is on **8080**. Nothing on 8087 → `fetch` rejects.
- Pointing at 8080 turned it into the real error: **403 "not a member of any workspace"** for the
  prefilled `ada`/`acme`.
- Root cause: the identity model keys on the **`user:<name>` principal** (token `sub`, membership
  row, `created_by`, seed `LB_SEED_USER=user:ada`), but `role/gateway/src/routes/login.rs` used the
  request `user` string **verbatim**. Bare `ada` was therefore a principal literally named `ada` — a
  *different* identity from the seeded `user:ada` — so `membership_login_resolve` refused it against
  the already-populated `acme`. It only ever worked against an empty in-memory `test_gateway`
  (where the stranger bootstraps as first member), which is why the preview looked fine until it met
  the user's persistent store.

**Fix (gateway login edge — canonicalize the handle):**
```rust
let principal = if req.user.starts_with("user:") { req.user.clone() }
                else { format!("user:{}", req.user) };
```
applied to `user_login_check`, `membership_login_resolve`, `dev_claims` (token `sub`), grant-resolve
(re-strips the prefix), and `LoginReply.principal`. `ada` and `user:ada` now resolve to the same
identity on any node; an empty node still bootstraps. Edge normalization only — no core membership /
`Subject` change, no extension branch.

**Follow-ons this exposed:**
- Preview default port + prefill moved **8087 → 8080** (the `make dev` node the user actually runs):
  `app/shell/src/lib/dev-defaults.ts` + `app/shell/web/index.web.tsx`. `?node=` still overrides for
  `make -C app dev`. The old "8087 dodges the 403" comment is corrected — the 403 is fixed at source.
- `app/sdk/tests/harness.ts` `addMember` passed a bare `"bob"` — which wrote a `bob` roster row AND
  skipped the member-role grant (`membership_add`'s `bare_user()` only grants for a `user:` sub). It
  "worked" only because the old `login("bob")` matched the bare row; once login canonicalizes to
  `user:bob` the mismatch surfaced as 403 in `channels.gateway.test.ts`. Canonicalized the harness to
  `user:bob` (the form `membership_add` documents), which also lands the grant.

**Verification:** `cargo test -p lb-role-gateway` incl. the new
`identity_routes_test::login_canonicalizes_a_bare_handle_to_the_user_principal`, and
`cargo test -p lb-host --test identity_membership_test --test authz_test` — green;
`app/sdk$ pnpm test:gateway` 17/17; Playwright e2e (`…5310/?node=http://127.0.0.1:8080`, bare `ada`)
logs in and shows the real channels (`#123`, `#abc`, `#general`); and a bare-`ada` login curl against
the user's **live `make dev` node** returned 200 with `principal: user:ada`.

## Gotcha for the next session

The shell consumes `@nube/app-sdk` as a pnpm `file:` dep that is **copied, not symlinked**. After
editing SDK source, `cd app/shell && pnpm install --ignore-workspace` to refresh the copy or the
shell typecheck/build sees the OLD SDK (`Property 'restore' does not exist` etc.). The gateway tests
(`app/sdk`) import `../src` directly and don't need this.
