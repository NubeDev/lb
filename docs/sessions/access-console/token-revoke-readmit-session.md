# A removed member can come back (session)

- Date: 2026-10-13
- Scope: access-console (live-token revoke) + global-identity (`membership.add` / `membership.remove`)
- Status: built + green; lands on the next `node-v*` tag

## Why

The live-token revoke marker (`token_revoke:<subject>`) was a presence-only record that nothing ever
deleted. Both `authz.revoke-tokens` and `membership.remove` write it, and the verify chokepoint refuses
EVERY token of a marked subject. So a member who was removed, or whose sessions an admin ended, could
never use that workspace again: re-adding them made a membership whose every token answered
`401 invalid or expired credential`. The table is reserved, so `store.delete` cannot clear it either.
Seen on a product node: a user re-added after removal signed in and got an empty app.

## What changed

- `lb-authz` `token_revoke`: the marker carries `revoked_at` (unix seconds, `#[serde(default)]`).
  `token_revoke_mark(…, now)` stamps it. New `token_revoked_for(store, ws, subject, issued_at)`: a
  token issued at or before `revoked_at` is refused, one issued after it is not; a marker with no stamp
  (written before this change) or a credential with no issue time (API key) is judged by presence, as
  before. New `token_revoke_clear` deletes a subject's marker (idempotent).
- `lb-host`: `revoke_tokens` and `membership_remove` take `now` and stamp the marker;
  `membership_add` clears the subject's marker (re-admission), so old undated markers are released by
  adding the person back. MCP callers pass the call's `ts`, else the host wall clock — never `0`, which
  would mean "refuse everything".
- Gateway `verify_token`: after the JWT signature check it reads the token's `iat`
  (`claims_unverified`, safe once verified) and asks `token_revoked_for`.

## Tests

- `authz/tests/access_console_test.rs`: cutoff (≤ refused, > allowed, API key by presence), undated
  marker refuses everything until cleared, clear is idempotent.
- `gateway/tests/access_console_routes_test.rs`: a sign-in after `revoke-tokens` works again; a removed
  member who is added back can use the workspace with their token.
- Existing gateway auth/membership suites and `host/tests/identity_membership_test.rs` green;
  `cargo check --workspace --all-targets` and `cargo fmt --check` clean.
