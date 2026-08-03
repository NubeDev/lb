# Legacy removal sweep — delete `POST /login` and the per-workspace `user` record — session

- Date: 2026-08-03
- Scope: ../../scope/auth-caps/email-login-scope.md (§"Pre-production — no legacy human path",
  §Sequencing — this session IS that tracked follow-up)
- Also amends: ../../scope/auth-caps/admin-crud-scope.md (the `user.*` half),
  ../../scope/auth-caps/global-identity-scope.md (decision #10, superseded)
- Debugging entry: ../../debugging/app/roster-login-disagree-legacy-user-rows.md
- Stage: post-S10 core-auth-caps
- Status: **done** — build/clippy/fmt green; test results below.

## Why now (the bug that forced it)

`GET /admin/members` in `acme` returned `user:ap`, but `GET /admin/identities/user:ap/workspaces`
returned `[]`, so `/auth/login` refused that person with **403 "not a member of any workspace"**.

Root cause was not a typo, it was a **second source of truth read three different ways**:

| reader | source | key |
| --- | --- | --- |
| `user_create` (write) | `user` table | **bare** handle (`"ap"`) |
| `membership_list` legacy pass | `user` table, **listed** | synthesized `"user:" + row.user` **field** → matched |
| `is_effective_member` | `user` table, **keyed read** | already-prefixed sub (`"user:ap"`) → missed |
| `has_any_effective_member` | `user` table, **listed** | (a third variant again) |

Aligning the key was available and rejected: it fixes the instance and keeps the class. This was the
**second** instance of the class (twin: `debugging/app/bare-login-handle-not-a-member.md`, a handle
canonical in one path and raw in another). lb is pre-production — there is nothing to migrate — so the
correct move is the one `email-login-scope.md` already mandated: **delete the legacy source.**

## What was deleted

**The legacy `user` record family (host):**
- `crates/host/src/users/` — the entire directory (`active`, `create`, `delete`, `error`, `list`,
  `login_check`, `model`, `mod`, `tool`), plus its `mod`/`pub use` in `lib.rs`.
- Caps `mcp:user.manage:call` + `mcp:user.disable:call` — dropped from `authz::builtin_roles::ADMIN_ONLY_CAPS`,
  from `nav::admin_lens::ADMIN_MARKER_CAPS` (`mcp:members.manage:call` is the People/roster marker now),
  and from the `workspace-admin` persona in `agent/personas/personas.toml`.

**The legacy human door (gateway):**
- `role/gateway/src/routes/login.rs` + its `/login` registration. Required, for two independent
  reasons beyond tidiness:
  1. **Account-enumeration oracle.** It resolved membership (`:84`) BEFORE checking the credential
     (`:97`), so any unauthenticated caller with a garbage password could distinguish member (401) /
     non-member (403) / disabled (a third message).
  2. **Self-promotion hazard.** `membership_login_resolve`'s bootstrap-on-empty made the first caller
     into an empty workspace a `workspace-admin`. Once legacy `user` rows stopped counting toward
     `has_any_effective_member`, a workspace populated only by legacy rows would have read *empty* —
     handing the next stranger the admin role.
- `role/gateway/src/routes/admin_users.rs` + the four `/admin/users*` registrations.
- `crates/host/src/membership/login_resolve.rs` (`membership_login_resolve`, `WORKSPACE_ADMIN_ROLE_CAP`) —
  verified `/login` was its only caller.
- **`role/gateway/src/session/credential.rs`** — the per-`(ws, user)` `CredentialCheck` seam
  (`DevTrustAny` / `PasswordHash` / `credential_check_from_env`), the `Gateway::credential_check` field,
  `with_credential_check`, and the builder wiring. **Not in the original plan**, but it existed solely to
  gate `POST /login`; leaving it would have left a live-looking `CredentialMode::PasswordHash` knob that
  nothing consulted. `DEV_LOGIN_ENV` + `CredentialRejection` moved into `session/global_credential.rs`
  (their only remaining consumer). `BootConfig::credential_mode` still selects the credential check —
  it now drives the single `GlobalCredentialCheck`.

## What was collapsed

- `membership/list.rs` — pass 2 (the legacy union) deleted; the roster is `membership` rows. (The
  tombstone check at `:39` was already dead code — `user_delete` wrote a `kind:"user-tombstone"` row
  that the `kind`-filtered list never returned.)
- `identity/workspaces.rs` — `is_effective_member` collapses to `raw::membership_is_member`;
  `has_any_effective_member` deleted with its only caller.
- `identity/login_workspaces.rs` — the `user_login_check` disabled-here filter deleted.
- Doc comments that named the legacy row/route across `membership/mod.rs`, `authz/revoke.rs`,
  `members/remove.rs`, `credential/verify.rs`, `identity/by_email.rs`, `identity_credential/*`,
  `invites/accept.rs`, `workspaces/register.rs`, gateway `state`/`server`/`health`/`node_identity`/
  `invite_accept`/`spa_fallback`/`mint_session`/`credentials`/`bin/test_gateway`, and node
  `config`/`builder`/`seed_identity`.

## Behaviour deliberately lost

**Per-workspace disable/enable disappears entirely.** There is no membership-row equivalent of
`active=false` and none was invented. The nearest surviving control is `membership.remove`, which is
strictly stronger: tombstone + `revoke_subject` + `token_revoke_mark` (the live token dies too).
Nothing silently no-ops — the verbs, caps and routes are gone, so a caller gets unknown-tool/404.
No caller was found that needs suspend-without-revoke. If one appears it is a new field on the
membership row, scoped on its own — not a resurrected parallel record.

## What was re-pointed (the cascade)

- **CLI** (`role/cli/`): `lb login` now posts `{email, password}` to `/auth/login` and, on the N>1
  branch, follows with `/auth/select {workspace}` using the select-token. `--user` → `--email` +
  `--password` (or `LB_LOGIN_PASSWORD`, so the secret stays out of shell history); `-w` became
  OPTIONAL (the 1-workspace auto-skip needs no pick, and with several the error NAMES them).
- **Test bootstrap**: new `role/gateway/tests/common/bootstrap.rs` — `provision_admin` /
  `provision_member` / `session_token`. It runs the same un-gated seams `seed_dev_identity` runs at
  boot (directory register → built-in role records → identity → membership → `member` +
  `workspace-admin` grants) and mints via `mint_full_session`, i.e. the **same function the live
  `/auth/*` routes call**. Explicit operator provisioning is now the only bootstrap, which is exactly
  the first-admin story `email-login-scope.md` already owned.
- **Suites ported**: `login_hardening_test` (escalation + member-reach, minus the `/login` credential
  case), `identity_routes_test`, `admin_routes_test`, `gateway_test`, `viewer_reach_test`,
  `nav_reach_test`, `email_login_test`, `email_login_deny_test`, `browser_session_test`,
  `browser_session_csrf_test`, `static_root_method_mismatch_test` (now probes `/auth/login` — the
  POST-only path that actually exists), `node/tests/credential_mode_test` (rewritten against
  `/auth/login` with a boot-seeded first admin), CLI `remote_test` + `config_persistence_test`.
- **Tests deleted** (they pinned removed behaviour): `identity_membership_test::legacy_user_rows_are_implicit_memberships_no_access_change`,
  `admin_crud_test::{disable_bites_login_and_enable_restores_and_list_hides_cred,delete_user_revokes_grants_and_blocks_login_idempotently}`,
  `admin_routes_test::{admin_can_create_disable_and_delete_a_user_over_the_routes,login_refuses_a_disabled_user_over_the_real_route}`,
  `identity_routes_test::{login_bootstraps_empty_workspace_and_refuses_a_non_member,login_canonicalizes_a_bare_handle_to_the_user_principal}`,
  `login_hardening_test::password_hash_gateway_401s_on_bad_or_absent_secret_and_isolates_by_workspace`.

## The new test

`crates/host/tests/identity_membership_test.rs::roster_and_login_path_agree_on_the_one_membership_source`
— real `mem://` store, real verbs, no mocks. It pins the invariant the bug violated, from **both**
directions: after `membership_add(ws, sub)` the roster (`membership_list`) contains `sub` AND both
readers of the login path (`identity_workspaces`, `login_workspaces`) contain `ws`; a sub with no
membership row is absent from all three; and `membership_remove` drops it from both in the same step.
It asserts the *agreement*, not either side in isolation — which is what the old union tests never did.

## Still standing (deliberately)

The per-workspace `credential` RECORD (`host/src/credential/`, `identity.set_credential`,
`credential_verify`) survives. It authenticates nothing — it is now only the invite flow's
**takeover-protection** state ("does this sub already hold a password in this workspace?",
`invites/accept.rs`). It is not a second login door. Removing it is the invite scope's call.

## Downstream: rubix-ai UI must drop two caps

`nav::admin_lens::ADMIN_MARKER_CAPS` is lockstep-tested against the rubix-ai UI's `ADMIN_SECTION_CAPS`
(`ui/src/lib/session/admin-caps.ts`, `admin-caps.lockstep.test.ts`). This session removed
`"mcp:user.manage:call"` from the lb array, so the UI must remove the SAME string from
`ADMIN_SECTION_CAPS` or the lockstep test fails. `"mcp:members.manage:call"` is already in both arrays
and becomes the People/roster admin marker — no UI addition is needed. The UI must also drop any
`mcp:user.disable:call` reference and stop calling `/admin/users*` (use `/admin/members`). That repo is
owned by another agent; this session did not touch it.
