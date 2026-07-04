# Preview reload after a gateway restart shows an empty channel list, not login

- Area: app
- Status: resolved
- First seen: 2026-07-04
- Resolved: 2026-07-04
- Session: ../../sessions/app/app-preview-stale-session-session.md
- Regression test: app/sdk/tests/restore-liveness.gateway.test.ts (+ e2e in the session doc)

## Symptom
In the RN-web browser preview, a user logs in (ada/acme), creates a channel, sees it — then
the preview gateway is restarted (any `make -C app dev` restart does this). On the next page
reload the shell shows the Channels screen with **"No channels yet — create one below."** The
user reads it as "the channels I just created vanished" and is stuck: the composer's
`channel_create`/`channel_list` calls quietly fail against a node that no longer knows them.

## Reproduce
1. `make -C app dev` (gateway on 8087, vite preview on 5310).
2. Open `http://127.0.0.1:5310/?node=http://127.0.0.1:8087`, Sign in, create a channel.
3. Restart just the gateway (`make -C app kill` then `make -C app gateway`, or a full
   `make -C app dev` restart).
4. Reload the browser tab. Observe the empty/stale Channels screen instead of a login prompt.

## Investigation
- The preview `test_gateway` boots **in-memory** (no `LB_STORE_PATH` → `Store::memory` in
  `host/src/boot.rs`) and mints a **fresh signing key every boot** (`role/gateway/src/
  signing_key.rs::resolve` falls to `SigningKey::generate()` when no durable store is paired).
  So a restart both wipes the channels AND invalidates every previously-issued token.
- The web preview persists the session token in `localStorage`
  (`app/shell/src/features/session/keychain.storage.web.ts`, key `lazybones.preview.sessions`)
  and the shell rehydrated it on boot via `client.session.restore()`
  (`app/shell/src/lib/client.ts`).
- `SessionStore.restore()` (`app/sdk/src/session/session.store.ts`) only *reads storage back*.
  It never contacts the node, so a dead token was rehydrated and presented as a live session.
  `App.tsx` then rendered the Channels stack over it. Staleness was only discovered later and
  *reactively* — whichever request happened to 401 — and during the restart window a request
  may not even 401 (the node is briefly down → `fetch` rejects, not 401), so `onAuthError`
  never fired and the empty state simply persisted.

## Root cause
`restore()` rehydrated a persisted token **without proving it still verifies** against the
node it names. A dead-but-well-formed token therefore drove a logged-in UI instead of a login
prompt. The 401→drop path (`request.ts` → `config.onAuthError` → `session.logout(ws)`) existed
but was only reached opportunistically by a later read, and not at all when the node was
unreachable rather than rejecting.

## Fix
Make rehydration **validated** at the SDK layer (`app/sdk/src/session/validate.ts` +
`app/sdk/src/client/create.ts`):

- New `client.restore()` rehydrates via the store, then probes once with
  `probeSession` → `GET /workspaces` (a route every member can call and the shell reads on
  boot anyway). It classifies the outcome, not the payload:
  - **401** → token no longer verifies → drop the session (fall to login).
  - **network error** → node unreachable → drop when `onUnreachable: "drop"` (the preview's
    setting: a throwaway in-memory node makes an unverifiable session worthless), or keep for
    a durable-node device build (`onUnreachable: "keep"`).
  - **anything else (403/2xx)** → the node authenticated the token → session is **live**,
    kept. A capability deny is not staleness.
- `app/shell/src/lib/client.ts` now calls `client.restore()` (with `onUnreachable: "drop"`)
  in place of the raw `session.restore()`. The drop calls `session.logout(ws)`, which notifies
  subscribers → `useSession` re-renders → `App.tsx` shows `LoginScreen`.

The fix lives in the SDK (not the shell) so the native build gets the same guarantee, and it
adds no core/gateway change — the gateway already 401s a dead token correctly.

## Verification
- `cd app/sdk && pnpm test:gateway` → 17/17 (4 new in `restore-liveness.gateway.test.ts`
  covering live-kept, dead-401-dropped, unreachable-dropped, unreachable-kept — all against the
  REAL spawned gateway, real tokens/verify path/network error; no fakes, rule 9).
- Playwright e2e (session doc): login → create `room-before-restart` → kill+reboot gateway on
  8087 → reload → asserted the shell shows **"Lazybones / Sign in"**, not the stale/empty room,
  and does not ghost the vanished channel. Console shows the real 401 the fix now catches.

## Prevention
A rehydrated session is never trusted until proven live. The class ("stored credential outlives
the server that issued it") is closed for any gateway the SDK talks to. A future durable preview
store (see the session doc's tradeoff note) would additionally keep the channels across restarts;
that is an enhancement, not required for correctness — `onUnreachable: "keep"` is the knob for
device builds that want offline sessions against a durable node.
