# Frontend — make collaboration real (session)

- Date: 2026-06-27
- Scope: ../../scope/frontend/collaboration-scope.md
- Stage: S7 — platform maturity (the frontend fit-and-finish carryover; STATUS.md "Next up" §5)
- Status: done

## Goal

Take the UI from a single-screen S2 demo bolted to fakes to a **real collaboration app over a real
node**, finishing both ends as needed: a real identity session (keystone), then workspaces, channels,
people + presence, inbox, and outbox status — each the channel's proven 4-file move (api client →
gateway route → host verb → view/hook). Resolve the parent `frontend-scope.md` open questions
(presence rendering, the demo principal / dynamic workspace).

## What changed

Built in the locked order (identity first). All four slices shipped end to end.

### Slice 1 — real session (the keystone)
- **Gateway**: deleted the demo principal in `role/gateway/src/state.rs`. `Gateway` now holds the
  node + the node's `SigningKey` + an injected clock. New `session/` module:
  `session/authenticate.rs` (bearer-token → `lb_auth::verify` → `Principal`; `verify_token` for the
  SSE `?token=` path), `session/credentials.rs` (the dev-login claim set — the one non-real piece).
  New `routes/login.rs` (`POST /login` → `lb_auth::mint` a real signed token; best-effort registers
  the workspace in the directory). **Every** other route now calls `authenticate` first — the
  workspace + caps come from the token, never the request (§7).
- **UI**: new `lib/session/` (`session.store.ts` — an observable holding token+workspace outside
  React so the IPC layer reads the token; `session.api.ts` `login`; `useSession.ts`;
  `session.types.ts`). `lib/ipc/http.ts` rewritten to attach `Authorization: Bearer` on every
  request and map every new command to its REST route. `channel.stream.ts` passes the token as
  `?token=` (EventSource can't set headers). `App.tsx` hardcoded `WS`/`CHANNEL`/`AUTHOR` gone —
  identity is `useSession`; logged out → `LoginView`.

### Slice 2 — workspaces + channels
- **Host**: new `channel_registry/` service (`channel_create`/`channel_list` reusing the channel
  `pub`/`sub` gate; `register_on_post` called from `channel::post` — create-on-first-post, additive
  + best-effort so a registry hiccup never fails a post). New `workspaces/` service
  (`workspace_create`/`workspace_list` over a reserved-namespace directory like the workflow
  directory, gated `mcp:workspace.*:call`).
- **Gateway**: `routes/workspace.rs`, `routes/channel_registry.rs`.
- **UI**: `features/workspace/` (switcher + `useWorkspaces`), `features/channel/ChannelList.tsx` +
  `useChannels.ts`. Switching workspace = re-login (the workspace is the token's hard wall).

### Slice 3 — people + presence
- **Host**: new `members/` service (`list_members`/`add_team_member` over the S4 `lb_assets`
  `relate`/`list_related` edges, gated `mcp:members.*:call` — the dedicated capability the S4 files
  scope flagged as a follow-up).
- **Gateway**: `routes/members.rs`.
- **UI**: `features/members/` (view + hook). **Presence rendered**: `features/channel/usePresence.ts`
  (a pure idempotent `mergePresence` reducer fed by the existing `event: presence` SSE feed) +
  a roster row in `ChannelView`.

### Slice 4 — inbox (real) + outbox status
- **Host**: new `inbox/` service (`list_inbox`/`resolve_inbox` over `lb_inbox`, gated
  `mcp:inbox.*:call`; the resolve actor is forced to the principal's `sub`). New `outbox/` service
  (`outbox_status`, read-only, gated `mcp:outbox.status:call`). Added `lb_outbox::delivered` scan.
- **Gateway**: `routes/inbox.rs`, `routes/outbox.rs`.
- **UI**: `features/inbox/` (the real durable inbox — replaces the workflow fake on the real path;
  Approve/Reject = the S6 gate as a UI action) and `features/outbox/` (read-only pending/delivered/
  dead-lettered groups).

### Fakes (tests only, contract-identical)
New `lib/ipc/{session,workspace,channelRegistry,members,inbox,outbox}.fake.ts`, chained in `fake.ts`,
workspace-scoped via the session store (mirroring how the real gateway derives ws from the token).
`channel_post` in the fake now also registers-on-post. `test/setup.ts` resets them all + the session.

## Decisions & alternatives (the scope's leans, all taken)

- **Login = real signed token now, dev credential store** (scope open q). Token path is real
  (`mint`+`verify`); the credential check is a dev-login (`dev_claims`) behind the same `verify` seam.
  Rejected building OAuth/OIDC now (the #1 scope risk — would swallow the slice).
- **Channel model = a registry record per `(ws, channel)`**, upserted on first post AND explicit
  create (scope lean). Rejected deriving the list from message rows (no metadata path, scan cost).
- **Session lives in `lib/session/` + `useSession`** (token + current workspace) (scope lean / parent
  open q). Held outside React in an observable store so `http.ts`/`channel.stream.ts` read the token.
- **Outbox = read-only status, workspace-scoped, capability-gated** (scope lean). No CRUD surface.
- **Teams UI = minimal** (list members, add member) (scope lean). Full team CRUD deferred.
- **SSE auth by `?token=` query param**, not a bearer header — `EventSource` cannot set headers; the
  stream route verifies the token identically. New decision forced by the transport; recorded here.
- **Switching workspace = re-login**, since the workspace is the token's hard wall (§7) — not a
  client-side toggle. New decision; recorded.
- **`workspace.create`/`workspace.list` gate on the session's own workspace.** The directory is
  node-level (a reserved namespace), but a caller must hold the verb in its workspace to touch it.
- Token-on-the-bus for a routed (cross-node) caller stays **deferred** (scope lock) — the gateway
  verifies the caller and the in-process node trusts that verified principal.

## Tests

Mandatory categories — all present **over the real route** (not a fake):
- **Session**: `login` issues a token that verifies; a **forged** token (wrong key) and an **expired**
  token are both `401`; the **workspace comes from the token, not the request body**.
- **Capability-deny**: ungranted `post` / `inbox_list` / `members_add` → `403` (the host's check).
- **Workspace-isolation, two real sessions on one node**: a ws-B token reads ws-A history / channel
  list / team members → empty, while ws-A sees its own — across gateway + store.
- **Channel registry**: `channel_create` then `channel_list` shows it; posting a new channel
  registers it; history unaffected.
- **Inbox**: `inbox_list` returns the real durable item; resolve/approve persists with the session
  principal as actor.
- **Outbox status**: reflects pending → delivered for a real effect.
- **Presence**: the `mergePresence` reducer is idempotent / order-independent.
- **Vitest view test per surface** (mirroring `ChannelView.test.tsx`) on the contract-identical fake:
  session login, ChannelList (+create-on-post +isolation), MembersView (+isolation), InboxView
  (+actor +isolation), OutboxView (pending→delivered +isolation), usePresence.

Note: `lb-host` test binaries depend on built wasm guests + the `echo-sidecar` native binary (CI
builds them first); built locally to run the full host suite. Two crates outside this slice
(`lb-tags`, `lb-ingest`) are a **concurrent session's in-flight work** (uncommitted untracked files)
and are excluded from this slice's verification — this slice touches neither.

### Rust — host collaboration service tests (cap-deny + ws-isolation)
```
running 5 tests
test each_verb_is_refused_without_its_grant ... ok
test channel_registry_is_workspace_isolated ... ok
test inbox_is_workspace_isolated_and_resolve_records_the_actor ... ok
test members_are_workspace_isolated ... ok
test outbox_status_is_workspace_isolated ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
```

### Rust — gateway, over the real session (session + deny + isolation + features + SSE)
```
running 5 tests   (tests/gateway_routes_test.rs)
test the_sse_stream_without_a_token_is_401 ... ok
test inbox_list_returns_real_items_and_resolve_persists ... ok
test channel_create_then_list_shows_it_and_posting_registers_a_channel ... ok
test the_sse_stream_authenticates_by_query_token_and_pushes_a_live_message ... ok
test outbox_status_reflects_pending_then_delivered ... ok
test result: ok. 5 passed; 0 failed; ...

running 9 tests   (tests/gateway_test.rs)
test a_request_without_a_token_is_401 ... ok
test a_post_without_the_grant_is_403 ... ok
test an_expired_token_is_rejected ... ok
test members_add_without_the_grant_is_403 ... ok
test a_ws_b_token_cannot_read_ws_a_channels_inbox_or_members ... ok
test the_workspace_comes_from_the_token_not_the_request ... ok
test a_forged_token_is_rejected ... ok
test inbox_list_without_the_grant_is_403 ... ok
test login_issues_a_token_that_authenticates_subsequent_requests ... ok
test result: ok. 9 passed; 0 failed; ...
```

### UI — Vitest (one view test per surface, all suites)
```
 Test Files  14 passed (14)
      Tests  40 passed (40)
```

### Gates
```
cargo build --workspace        → Finished (green)
cargo fmt --all --check        → clean
rust/scripts/check-file-size.sh → all source files within 400 lines (404 checked)
pnpm build                     → tsc --noEmit + vite build green
```

## Debugging

None — nothing broke that needed a `debugging/` entry. (One self-inflicted test-assertion fix:
`OutboxView.test.tsx` initially used `.closest("div")` which matched the title div, not the group;
switched to asserting the `Pending · N` / `Delivered · N` counts. Trivial, no regression entry.)

## Public / scope updates

- Promoted to `public/frontend/collaboration.md` (the shipped collaboration surfaces + the session
  contract) and updated `public/frontend/frontend.md` to point at it.
- `scope/frontend/collaboration-scope.md` open questions resolved (login mechanism, channel model,
  token transport [deferred], outbox visibility, teams granularity, session location) — recorded in
  the scope doc's Open questions section.
- `STATUS.md`: new shipped slice row + the fit-and-finish carryover items (presence rendering, real
  login session) marked done.

## Dead ends / surprises

- `EventSource` cannot set an `Authorization` header — forced the `?token=` query-param auth on the
  SSE stream route. Faithful (same `verify`), but a real deployment should prefer short-lived stream
  tokens / a cookie.
- The store `list` is a pure equality filter (no table dump), so the channel registry + workspace
  directory carry a constant `kind` discriminant to enumerate them — the same trick the workflow
  directory uses with `status`.
- A concurrent AI session split `gateway_test.rs` into `gateway_test.rs` + `gateway_routes_test.rs`
  (+ shared `tests/common/`) to stay under the 400-line limit, and is independently building the
  `tags`/`ingest` crates. Cooperated: kept my files coherent with the split.

## Follow-ups

- Token-on-the-bus: the hub re-verifying a **routed** caller's grant (cross-node) — deferred (scope).
- Real IdP behind the `verify` seam (password / OIDC) — replaces the dev credential store.
- Tauri desktop path: this slice wired the **browser/gateway** path; the Tauri command layer still
  fixes its workspace in `src-tauri/src/state.rs` — a follow-up mirrors the session there.
- Full team CRUD; workspace provisioning (this only makes a workspace *listable*, not its data).
- STATUS.md updated (last step). ✅
