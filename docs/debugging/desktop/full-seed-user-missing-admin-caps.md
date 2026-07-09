# `full` desktop: seeded `user:ada` logs in but is missing access to (almost) everything

**Symptom.** The `full` standalone desktop binary (`make windows-full` / `make linux-full`)
boots, prints `full: loopback gateway on http://127.0.0.1:8800 (login as user:ada / acme)`,
and login succeeds — but the seeded user can't reach most of the app. Every admin surface and
most authoring surfaces 403. The user *looks* provisioned (login works, a viewer-level read like
`tools.catalog` returns) but has almost no reach.

**Cause.** `ui/src-tauri/src/full.rs::seed_dev_identity` assigned the seeded user the built-in
role grants — `grant_assign(… "role:member")` and `grant_assign(… "role:workspace-admin")` — but
**never seeded the built-in role RECORDS**. A role grant resolves to caps only if the role record
(carrying the cap bundle) exists in the workspace: `resolve_caps` reads the `role` rows and folds
their caps into the token. With no `workspace-admin` record, the grant resolved to an **empty cap
set**, so the token minted at `/login` carried only the `dev_claims` base + viewer floor — not the
member/admin bundles. Hence "logged in, but missing access to everything."

The dev node (`rust/node/src/main.rs:31`) already does this correctly: it calls
`lb_host::ensure_builtin_authz_roles(store, ws)` **before** the grants, which seeds the
`viewer`/`member`/`workspace-admin` records with their bundles (`builtin_roles.rs`). `full.rs` — a
near-verbatim copy of that seeder — had dropped that one line.

Note this was masked by the existing `full_loopback_test`: it only asserted `tools.catalog` (a
viewer-level read), which passes regardless of the role fold, so the gap shipped green.

**Fix.** Add the missing seed to `full.rs::seed_dev_identity`, mirroring `node/main.rs`:

```rust
lb_host::ensure_builtin_authz_roles(store, ws)
    .await
    .map_err(|e| e.to_string())?;
```

before the `identity_create` / `membership_add_raw` / `grant_assign` calls. Idempotent (only
writes a role record when absent), so re-running every boot is a cheap no-op.

On a single-workspace standalone desktop node, `workspace-admin` (now properly seeded) is the
"everything" role — it carries every admin cap (membership/roles/grants/extensions/workspace ops).
`super-admin` is the reserved cross-workspace node-operator tier and is not seeded per-workspace,
so it adds no reachable surface here.

**Verified.** Rebuilt `make linux-full`; booted under xvfb; decoded the login token → **220 caps
incl. all 10 admin caps** (`members.add`, `workspace.delete`, `grants.assign`, `roles.*`,
`user.manage`, `ext.uninstall`, `native.install`, `authz.resolve`). Drove the real admin route
`GET /admin/roles` → **HTTP 200** listing `[member, viewer, workspace-admin]` (before the fix: 403,
no role records). Then rebuilt `make windows-full` with the fix.

**Regression guard.** `ui/src-tauri/tests/full_loopback_test.rs::seeded_user_resolves_to_a_real_workspace_admin`
— logs in as the seeded user and drives `GET /admin/roles` (admin-gated `mcp:roles.list:call`);
asserts 200 + that the `workspace-admin` record is seeded. Fails (403) without the fix; passes with
it. Run: `cd ui/src-tauri && cargo test --features full --test full_loopback_test`.
