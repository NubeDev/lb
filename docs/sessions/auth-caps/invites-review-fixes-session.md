# Invites — peer-review fixes session

- Date: 2026-07-11
- Scope: `docs/scope/auth-caps/invites-scope.md`
- Follows: `invites-session.md` (the original slice)
- Status: done

## Goal

Land the confirmed findings from the peer review of the invites slice: the accept-race /
credential-ordering bug, the missing day-one rate limit on the public route, the never-exercised
real-relay email path, the scope-doc/code contradiction on no-escalation, resend's stale expiry +
write ordering, the missing takeover/first-call tests, and warning cleanup.

## What changed

### 1. Accept atomicity + credential race (the bug — see debugging entry)
- `rust/crates/authz/src/invite.rs` — `invite_mark_accepted_raw` claims redemption via a
  store-level CAS: `lb_store::create` of an `invite_claim:{hash}` row (`INVITE_CLAIM_TABLE`);
  first write binds, every racer gets `StoreError::Conflict` → `false`. New
  `invite_release_claim_raw` (winner-only rollback to `pending`).
- `rust/crates/host/src/invites/accept.rs` — reordered: token + takeover checks (reads only) →
  **claim** → `onboard()` mutations (identity/credential/membership/grants; one rollback site
  releasing the claim on `Err`) → mint. Loser of a double-redeem is rejected before any
  credential mutation.
- Debug history: `docs/debugging/auth-caps/invite-accept-credential-race.md` (+ README row).

### 2. Rate limit on `POST /public/invite/accept`
- New `rust/role/gateway/src/routes/rate_limit.rs` — `FixedWindowLimiter` (10 req / 60s per
  client key; key = first `x-forwarded-for` hop, else a shared `"direct"` bucket — degrades
  tighter, never looser) + `invite_accept_rate_limit` axum middleware. 429 on excess.
- `server.rs` — the layer applied to the public invite route ONLY (one-liner mount).
  *Rejected:* tower `RateLimitLayer` — global (not per-client) and buffers instead of rejecting.

### 3. EmailTarget through the REAL relay
- New `rust/crates/host/tests/invite_email_relay_test.rs` — `invite.create` → `relay_outbox`
  (the exact loop `spawn_relay_reactors` ticks) → `EmailTarget` → `RecordingEmailProvider`;
  asserts delivery, token in body, idempotent second pass, ws-isolation of the relay.
- `email_target.rs` — blanket `EmailProvider for Arc<P>` so a test can observe the recorder the
  target owns. `lb_host` now re-exports the email-target types.
- Scope doc now states the **wiring contract** explicitly: no `spawn_relay_reactors`
  registration by the product host ⇒ no delivery, ever.

### 4. No-escalation decision recorded
- Kept the code's `grants.assign`-precedent behavior (role grants exempt from a minter
  holds-cap check). Scope doc Goals/Testing plan amended with the resolution AND the rejected
  alternative (minter-bound check closes nothing — the minter can `grants.assign` post-join —
  while breaking delegated onboarding admins). Doc no longer contradicts the code.

### 5. Resend: expiry refresh + write ordering
- `rust/crates/host/src/invites/revoke.rs` — resend now refreshes expiry (original TTL measured
  from `now`; `expires_ts == 0` stays never-expiring) and writes **new-before-old** (enqueue new
  invite + email effect atomically, revoke the old record last) — a mid-way failure leaves at
  worst both-pending-briefly, never zero pending invites.

### 6. Workspace-local takeover check (deferred, documented)
- Credentials are per-`(ws, sub)` while identity is global, so accept's `current_secret` check
  only proves the inviting workspace's credential row. Recorded as an open question in the scope
  doc, cross-referenced to `login-hardening-scope.md` (credential placement). No restructure now.

### 7. Missing tests from the plan
- New `rust/crates/host/tests/invites_hardening_test.rs` (4 tests): double-redeem loses before
  credential mutation; existing identity requires `current_secret` (missing/wrong → 409 path,
  credential untouched, invite still pending; correct → binds, exactly one identity);
  accept-then-first-call makes a REAL cap-gated call (`list_members` via `mcp:members.list:call`
  with the minted token); resend refreshes expiry / old dead / new works past original expiry.
- New `rust/role/gateway/tests/invite_rate_limit_test.rs` — real router: MAX hits reach the
  handler, next hit 429, different client unaffected (window-roll tolerant, never flaky).

### 8. Warning cleanup
- Unused imports removed in `invites/accept.rs`, `invites/create.rs`, `invites/mod.rs` —
  `lb-host` builds with zero invites warnings.

## Test results (green)

```
invites_test.rs            11 passed (incl. mandatory cap-deny + ws-isolation, unchanged)
invites_hardening_test.rs   4 passed
invite_email_relay_test.rs  1 passed
invite_rate_limit_test.rs   1 passed (gateway route)  + 3 limiter unit tests (lib)
cargo build -p lb-role-gateway / -p lb-host: clean
```

Pre-existing failures not chased (per project memory): `agent_persona_catalog_test`,
`agent_routed_test`.
