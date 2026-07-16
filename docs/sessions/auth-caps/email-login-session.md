# Email/password login + workspace selection (the Slack front door) — session

- Date: 2026-07-16
- Scope: ../../scope/auth-caps/email-login-scope.md
- Downstream (UI half): rubix-ai `docs/scope/frontend/email-login-scope.md`
- Stage: post-S10 core-auth-caps (builds on shipped global-identity + login-hardening)
- Status: **in-progress** — Phase 1 (backend) landed additively + green; the `/login` removal sweep is a tracked follow-up (see Sequencing).

## Goal

Ship the backend half of the Slack-style human front door: a globally-unique email + one global
argon2id password on the `_lb_identity` identity, admin verbs to set them, and the three gateway
routes `POST /auth/login|select|switch` (+ self-service `POST /auth/password`) implementing the
authenticate-once-then-choose 0/1/N branch. Keep the token contract untouched (one token, one `ws`),
mirror the shipped credential-check pluggable pattern, and land the scope's full test plan on the real
gateway + store (argon2 real, no mocks).

## Decisions (resolved open questions)

1. **`/auth/login` honors `LB_DEV_LOGIN`** → yes. `GlobalCredentialCheck` is a trait selected by the
   same env as the per-ws seam: set → `GlobalDevTrustAny` (password-less, dev/CI), unset →
   `GlobalPasswordHash` (argon2). CI for the new flow needs no seeded hashes.
2. **Rate limit** → 10 failures / 15 min per email, per-email only. Reuses `FixedWindowLimiter` with a
   new `peek` (check-without-count) so only *failed* attempts count toward lockout.
3. **`workspaces[]` carries `{ws, name}`** → yes, reusing `IdentityWorkspace`.

## Sequencing — build-then-remove (why `/login` still exists)

The scope's end-state removes `POST /login`, the per-ws `Credential`, and `identity.set_credential`
(machines move to API keys). That removal cascades into the CLI login, the invite-accept mint, and
~10 session-seeding test suites. To keep the tree green at every step, this session landed the
**additive** replacement — the global credential + `/auth/*` — **alongside** the existing `/login`,
fully tested. The **removal sweep** (delete `/login` + per-ws credential + `identity.set_credential`;
re-point the CLI + machine callers to API keys; retire/port the legacy suites) is the immediate next
slice, tracked in the scope's "Sequencing" note and STATUS. Until it lands, the two credential stores
coexist and never consult each other.

## What changed

### `lb-authz` (raw store layer)
- **`identity.rs`** — `Identity` gains an optional lower-cased `email`; `fold_email` (trim +
  lowercase, the one canonicalizer); `identity_set_email` + `identity_create_with_email` claim a
  **race-safe unique index** via the new `identity_email:{folded}` reverse-index table — uniqueness is
  enforced by `store::create` (Conflict-on-duplicate), not read-then-write; `identity_by_email`
  resolves email→sub case-insensitively. `identity_create` preserves an existing email on idempotent
  re-create.
- **`identity_credential.rs`** (new) — the global credential record `identity_credential:{sub}` =
  `{sub, kind:"password", phc, set_ts}` in `_lb_identity`; `identity_credential_set` /
  `identity_credential_phc` (the only reader; the hash is never returned to a caller).

### `lb-host` (services)
- **`identity_credential/`** (new service, one concern/file) — `set` (admin `identity.set_password`,
  gated `mcp:identity.manage:call`), `change` (self-service verify-old-set-new), `verify`
  (`global_credential_verify`, **timing-uniform**: argon2 burned against a process-wide dummy hash on
  an unknown/absent credential so an unknown email can't be told from a wrong password by latency),
  `tool` (MCP bridge). Reuses the shipped argon2 `hash_secret`/`verify_secret`.
- **`identity/`** — `IdentityView` carries `email`; `identity.create` accepts an `email` arg;
  `set_email` verb; `by_email` login-path lookup (un-gated); **`login_workspaces`** — the un-gated
  membership enumeration for `/auth/login` (effective member AND not disabled there, `{ws, name}`
  rows). New `IdentityError::EmailTaken` → 409.
- `tool_call.rs` — `identity.set_email` / `identity.set_password` gate on `mcp:identity.manage:call`;
  `identity.set_password` dispatches to the new bridge.

### `lb-role-gateway`
- **`session/global_credential.rs`** — the `GlobalCredentialCheck` trait + `GlobalPasswordHash` /
  `GlobalDevTrustAny`, selected by `LB_DEV_LOGIN` (mirror of the per-ws seam). Held on `Gateway`
  (`with_global_credential_check`, env-wired in `boot`).
- **`session/select_token.rs`** — the powerless select-token: `ws:""`, `caps:[]`,
  `constraint:["ws-select"]`, ~5-min TTL. `mint_select_token` + `is_select_token` (the ONE positive
  acceptor; every normal gate refuses it structurally on empty ws + empty caps).
- **`session/mint_session.rs`** — `mint_full_session`, the ONE role-correct issuance path (viewer
  floor ∪ `resolve_caps_live` ∪ nav-reach + best-effort directory register), factored out of
  `routes/login.rs` so `/login` AND all `/auth/*` mint byte-identically. `login.rs` now calls it.
- **`routes/auth_login|select|switch|password.rs`** + `auth_reply.rs` (the `AuthReply` envelope:
  full-session OR select-needed, both carrying the roster). `/auth/login` runs the rate-limit →
  email→sub → timing-uniform verify → uniform 401 on failure → `login_workspaces` → 0/1/N branch.
  `/auth/select` is the select-token's sole acceptor (re-checks membership). `/auth/switch` re-mints
  password-less into another member workspace (re-checks membership + disabled). `/auth/password` is
  the self-service change (token's own sub, verify old). Admin REST: `/admin/identities` accepts
  `email`; `/admin/identities/{sub}/email` + `/{sub}/password`.
- `routes/rate_limit.rs` — the per-email login limiter (`auth_login_allowed`/`_record_failure`) +
  `FixedWindowLimiter::peek`.

## Tests (real gateway + SurrealDB, argon2 real, no mocks) — GREEN

Backend:
- `role/gateway/tests/email_login_test.rs` **6** — 1-branch auto-skip (full token, no select-token) ·
  N-branch (select-token + sorted roster → `/auth/select` mints) · 0-branch (403, no token) ·
  **unknown-email and wrong-password return the IDENTICAL 401 body** · email uniqueness (case-folded
  409) + case-insensitive lookup · self-service change (wrong-old 401, rotate, old-dead/new-works).
- `role/gateway/tests/email_login_deny_test.rs` **3** — select-token refused by admin route + MCP call
  + data write, accepted ONLY at `/auth/select` · a full token refused at `/auth/select` · switch to a
  non-member ws is 403, switch into a member ws re-mints.
- Unit: `select_token` **3** (powerless + recognized · expiry · a full token is not a select-token) ·
  `rate_limit::peek` **1**.

Regression (the `login.rs` refactor + identity signature change must not move anything):
- `login_hardening_test` 5 · `identity_routes_test` 9 · `gateway_test` 6 · `admin_routes_test` 3 ·
  `nav_reach_test` 2 · `viewer_reach_test` 2 · gateway `--lib` 17 — all green.
- `lb-authz --lib` 20 · `lb-host --lib` 259 · `lb-host --test credential_test` 3 — green.
- `cargo build --workspace` green (CLI + node + all downstream compile with the new host/authz APIs).

Command output pasted at the bottom of this doc.

## Non-goals / deferred

- The `/login` removal sweep (see Sequencing) — next slice.
- Email verification / password-reset email / MFA — unchanged deferrals.
- OIDC provider — the `GlobalCredentialCheck` seam leaves room; no provider here.
- Phase 2 (rubix-ai `[patch]` wire) + Phase 3 (rubix-ai UI) are downstream, in the rubix-ai repo.

## Green output

```
# gateway email-login suites
cargo test -p lb-role-gateway --test email_login_test --test email_login_deny_test
  email_login_deny_test: 3 passed
  email_login_test:      6 passed
# regression
cargo test -p lb-role-gateway --test login_hardening_test --test identity_routes_test \
  --test gateway_test --test admin_routes_test --test nav_reach_test --test viewer_reach_test
  5 / 9 / 6 / 3 / 2 / 2 passed
cargo test -p lb-role-gateway --lib   -> 17 passed
cargo test -p lb-authz -p lb-host --lib -> 20 / 259 passed
cargo test -p lb-host --test credential_test -> 3 passed
cargo build --workspace -> Finished (exit 0)
```
