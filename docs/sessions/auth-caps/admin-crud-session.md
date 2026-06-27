# Auth-caps — admin CRUD: the destructive half + user lifecycle (session)

- Date: 2026-06-27
- Scope: ../../scope/auth-caps/admin-crud-scope.md
- Stage: S9+ — slice 2 of 4 (admin-CRUD/lifecycle/console). Builds on slice 1 ([authz-grants](authz-grants-session.md)).
- Status: done (backend + gateway routes + http.ts + fakes; the console UI is slice 4)

## Goal
Take the management verbs from "create/list only" to a full administered surface: the **destructive
half** (workspace/team/member **delete·rename·remove** + a guarded **hard purge**) and a real
**dev-store user CRUD** with the **login active-check** (`disable` bites minting). Consume slice 1's
two seams — `resolve_caps` is the login projection (wired in a later pass; the dev claim set already
carries the admin caps) and `revoke_subject` is called on `user.delete` / `teams.delete`. Expose every
verb over the gateway (+ `http.ts` + a 1:1 fake) so the browser console (slice 4) can drive it.

## What changed
**New host `users` service** (`crates/host/src/users/`) — the genuinely-missing primitive:
- `model.rs` — `UserRecord{user,active,role,cred_ref,kind,ts}` (per-`(ws,user)`, one global principal
  id) + a credential-free `UserView` (`cred_ref` is **never** serialized out). Tombstone kind for delete.
- `create`/`list`/`active`(disable+enable)/`delete`/`login_check`/`tool`. Gated `mcp:user.manage:call`
  (CRUD) / `mcp:user.disable:call` (disable). `user_delete` tombstones + calls `revoke_subject` (slice 1).
- `user_login_check` — the **un-gated pre-mint seam** (no principal yet): absent record → allowed
  (auto-seed preserved); present record must be non-tombstoned + `active`, else `Disabled`.

**Workspace lifecycle** (`crates/host/src/workspaces/`) — `rename` (+ un-archive), `delete` (soft
archive, hidden from `list`), `purge` (hard): purge needs the **distinct `mcp:workspace.purge:call`
cap AND a typed `confirm` token = the ws id** (both gates). Added `WorkspaceStatus` + a directory
`TOMBSTONE`; `list` now hides archived; `create` refuses to resurrect a tombstoned (purged) ws.

**Teams destructive** (`crates/host/src/teams/`) — `teams.delete` (cascade: drop member edges +
`revoke_subject(Team)` + tombstone the record, one op, returns members-removed count) + `teams.rename`.
Gated `mcp:teams.manage:call`. Added `team_delete` to `lb-authz`.

**Members** — `remove_member` (`crates/host/src/members/remove.rs`), the missing destructive member
verb, gated `mcp:teams.manage:call`, idempotent.

**Gateway** (`role/gateway/`) — the **login active-check** wired into `POST /login` (a disabled/deleted
user → `403`, the one session-path edit); dev claim set extended with the admin caps so the demo
principal is a workspace admin (the gateway re-checks every verb — UI gate is convenience). New routes:
`admin_users`/`admin_teams`/`admin_workspaces`/`admin_members`/`admin_grants` (read+assign/revoke), all
mounted under `/admin/*` + `DELETE /teams/{team}/members/{user}`. Each re-checks the cap server-side.

**UI transport** — `ui/src/lib/ipc/http.ts` gained every new verb (no more `unknown command` in the
browser) incl. a `delJson` helper; `admin.fake.ts` (+ `members_remove` and helpers in `members.fake.ts`)
mirrors the routes 1:1 for Vitest.

## Decisions (open questions resolved)
- **Hard-delete = purge cap + typed confirm token** (both, the scope's lean). Soft archive is the
  default; rename un-archives.
- **`user.delete` is workspace-scoped** (per-ws record + `revoke_subject` for that ws); global removal
  is a separate node-directory action, not this verb.
- **Tombstones win over resurrection** — `workspace_create`/`workspace_rename` no-op on a purged ws;
  `user`/`team`/`workspace` deletes are tombstone-upserts (sync-idempotent §6.8), never row-deletes.
- **Login auto-seed preserved** — an un-administered workspace (no user record) still mints; only an
  explicit disabled/deleted record refuses.
- **Freshness asymmetry** surfaced on `members.remove` / `teams.delete`: Gate-3 live, Gate-2 on re-mint.

## Tests (green)
Host — `cargo test -p lb-host --test admin_crud_test` (7):
```
denies_destructive_verbs_without_their_cap ... ok            (capability DENY, per verb)
hard_delete_needs_the_purge_cap_above_the_soft_cap ... ok    (soft-before-hard + confirm token)
ws_b_admin_cannot_touch_ws_a ... ok                          (two-workspace ISOLATION)
disable_bites_login_and_enable_restores_and_list_hides_cred ... ok
delete_user_revokes_grants_and_blocks_login_idempotently ... ok
teams_delete_cascades_members_and_revokes_grants ... ok
workspace_soft_then_hard_and_tombstone_not_resurrected ... ok
test result: ok. 7 passed; 0 failed
```
Gateway — `cargo test -p lb-role-gateway --test admin_routes_test` (3):
```
forged_admin_call_by_non_admin_is_denied_server_side ... ok  (server deny on a forged call — the UI
                                                              gate is NOT the boundary)
admin_can_create_disable_and_delete_a_user_over_the_routes ... ok
login_refuses_a_disabled_user_over_the_real_route ... ok     (disable bites login over the transport)
test result: ok. 3 passed; 0 failed
```
UI — `npx tsc --noEmit` clean; `npx vitest run` → **40 passed** (no regression; admin fake + http.ts
typecheck). `cargo build --workspace` green; `cargo fmt --check` clean; all files ≤290 lines.

Pre-existing unrelated failure: `github_bridge_normalize_test` needs a prebuilt `github_bridge_ext.wasm`
not present in this checkout (environmental, not touched by this slice).

## Follow-ups
- **Slice 3 (extensions lifecycle):** enable/disable/uninstall/list/start/stop/restart + boot reconciler
  + registry publish, over the gateway.
- **Slice 4 (UI):** `features/admin` (the tables + `ConfirmDestructive`) + `features/extensions`,
  consuming the http.ts verbs added here; retire `RegistryView`/`NativeView`.
- Wire `resolve_caps` into `dev_claims` so a login token's caps come from the grant store (the dev set
  is a superset today); seed the built-in roles. Audit hook (`deleted_by/at`) is noted, not built.

## Related
- scope: ../../scope/auth-caps/admin-crud-scope.md ; slice 1: authz-grants-session.md
- the UI that drives these: ../../scope/frontend/admin-console-scope.md
- the login path amended: ../../scope/frontend/collaboration-scope.md
