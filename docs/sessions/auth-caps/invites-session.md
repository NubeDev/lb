# Invites — session

- Date: 2026-07-11
- Scope: `docs/scope/auth-caps/invites-scope.md`
- Status: done

## Goal

Token onboarding for people who don't exist yet: an admin mints an invite (carrying role/team
intent + opaque payload), the outbox delivers the email, and a pre-auth accept route redeems the
token into `identity` + `membership` + grants — atomically, with caps live on first login.

## What changed

### `lb-authz` crate
- **`Invite` record** (`invite.rs`): workspace-scoped, single-use. Fields: `token_hash` (SHA-256
  of the raw token, also the record id for O(1) lookup), `email`, `role`, `team`, `payload`
  (opaque, rule 10), `status` (pending→accepted|revoked|expired), `minter`, `created_ts`,
  `expires_ts`, `accepted_by`, `accepted_ts`. Raw verbs: `invite_create_raw`, `invite_get_raw`,
  `invite_list_raw`, `invite_revoke_raw`, `invite_mark_accepted_raw`.

### `lb-host` crate
- **`invites/` module** (7 files, one verb per file):
  - `token.rs` — `lbi_`-prefixed 32-byte Crockford-base32 token; SHA-256 hash (fast hash correct
    for full-entropy input, same reasoning as apikeys).
  - `create.rs` — `invite_create` (gated `mcp:invite.create:call`; enqueues email effect
    transactionally with the invite record via `lb_outbox::enqueue`).
  - `list.rs` — `invite_list` (gated `mcp:invite.list:call`).
  - `revoke.rs` — `invite_revoke` + `invite_resend` (gated `mcp:invite.create:call`; resend
    rotates the token, keeps the record).
  - `accept.rs` — `invite_accept` (pre-auth; the atomic onboarding chain: verify token →
    create-or-match identity by email → set credential → membership.add → apply role/team grants
    → mark redeemed → mint session). Prevents email-match takeover (existing identity with a
    credential must provide `current_secret`).
  - `tool.rs` — MCP dispatch for admin verbs.
  - `error.rs` — `InviteError` → `ToolError` mapping.
- **`outbox/email_target.rs`** — the email `Target` adapter + `EmailProvider` trait (the one
  sanctioned external, one named file) + `RecordingEmailProvider` (test fake).
- **`tool_call.rs`** — `"invite."` added to `HOST_NATIVE_PREFIXES` + dispatch branch.
- **`authz/builtin_roles.rs`** — `mcp:invite.create:call` / `mcp:invite.list:call` in admin-only.
- **`system/catalog.rs`** — invite verbs in the host inventory.

### Gateway
- **`POST /public/invite/accept`** — the third public route (besides `/login` and `/hooks`).
  Pre-auth; token-gated; the gateway's signing key mints the session.

## Decisions & alternatives

1. **Email-as-sub.** The invitee's `sub` is `user:<email>` (e.g. `user:sam@example.com`). This
   avoids adding an `email` field to the `Identity` struct and means the invite email IS the login
  handle. *Rejected:* adding an email field to Identity + a lookup index — more surface for no
   real benefit; the email is already unique per invite.

2. **Role grants exempt from no-widening (consistent with `grants.assign`).** The existing
   `grants_assign` exempts `role:` caps from the `holds_cap` check (the role's caps were bounded
   at `roles.define` time). Invites follow the same precedent: `mcp:invite.create:call` IS the
   authority. *Rejected:* checking the minter's grant store for `role:<name>` — inconsistent with
   `grants.assign`, and token-only caps (the test pattern) can't be checked against the store.

3. **`accept` is a gateway route, not an MCP verb.** The accept chain mints a session token (needs
   the signing key) and has no principal (it's pre-auth). It doesn't fit the MCP dispatch model.
   *Rejected:* making `accept` an MCP verb — would require passing the signing key through the
   tool dispatcher, which is a gateway concern, not a host-tool concern.

4. **`invite.accepted` bus event deferred (v1).** The scope asked whether the bus event rides in
   v1 or polling is enough. Polling the invite record (or the `invite.list` verb) is sufficient
   for v1 — the extension observes acceptance by checking `status == accepted` + `accepted_by`.

5. **`max_uses > 1` deferred.** Strictly single-use first (the scope's recommendation). A
   "join link" mode (`max_uses`) is a later additive field on the same record.

6. **Accept UI lives in the minimal shell.** Coordinated with `minimal-shell-scope.md` — the
   accept screen is a themed client route (`/accept?token=…`) that calls `POST /public/invite/
   accept`. It's the natural home (the shell is the auth-screens package).

## Tests

Real store, real resolver, real capability gate — no mocks (rule 9). The email provider is the
one sanctioned fake (`RecordingEmailProvider`, testing-scope §0).

### `lb-host` integration tests (`tests/invites_test.rs` — 11 tests)

- `create_and_list_invite` — happy path
- `accept_invite_onboards_new_member` — the atomic chain: identity + membership + grants + session
- `double_redeem_is_rejected` — second accept → `AlreadyAccepted`
- `expired_invite_is_rejected` — past `expires_ts` → `Expired`
- `revoke_then_accept_is_rejected` — revoke via MCP → accept → `Revoked`
- `denies_create_without_invite_create_cap` — **mandatory capability deny**
- `denies_list_without_invite_list_cap` — **mandatory capability deny**
- `invite_not_visible_from_other_workspace` — **mandatory workspace isolation**
- `accept_with_wrong_workspace_fails` — **mandatory workspace isolation**
- `admin_can_invite_with_any_role` — role grants follow the grants.assign precedent
- `bad_token_is_rejected` — malformed token → `BadToken`

### Existing tests (no regressions)

- All authz/admin_crud/scoped_grants tests green.
- Built-in role tier lattice holds (invite caps in admin-only set).
- System catalog prefix coverage holds.

## Debugging

**Invite record overwritten by outbox enqueue.** The initial `invite_create` called
`invite_create_raw` (writes the invite) then `lb_outbox::enqueue(..., &json!({}), &effect)` which
overwrote the invite record with an empty object in the same table+id. Fixed by passing the invite
record value as the `change` parameter to `enqueue` (the apikey-create precedent: the change row IS
the domain record, written atomically with the effect).

## Public / scope updates

Scope open questions resolved (see Decisions above):
- ✅ `invite.accepted` rides polling for v1 (bus event deferred).
- ✅ `max_uses > 1` deferred (strictly single-use first).
- ✅ Accept UI lives in the minimal shell.

## Follow-ups

- Wire `spawn_relay_reactors` with the `EmailTarget` at boot (the product host supplies the real
  `EmailProvider` impl; the test impl records sends).
- Rate-limiting on `POST /public/invite/accept` (the scope's risk note — ships from day one when
  the route is exposed publicly).
- The accept UI screen in the minimal shell.
