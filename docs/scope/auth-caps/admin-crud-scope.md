# Auth-caps scope — admin CRUD: the destructive half (workspace · user · team · member delete/disable/remove)

Status: scope (the ask). Promotes to `public/auth-caps/` once shipped. The completing sibling of
`authz-grants-scope.md`. That scope builds the **durable grant/role/team model** and the *create/assign*
verbs; this scope builds the **destructive and user-lifecycle half** the platform is missing — the verbs
that **delete, disable, remove, and rename** — plus a real **user record CRUD** over the existing
dev-credential store. Together they make identity, tenancy, and membership a complete administered surface
instead of the create-only, no-delete state shipped in S9.

Today the management verbs are **half a CRUD**. Workspaces have `create`/`list` but no rename/delete.
Members have `add`/`list` but no remove. There is **no team service at all** (teams are implicit
membership edges; `authz-grants` promotes them, but even there create/add exist before
remove/delete). There are **no user records** — login auto-seeds a principal from a dev credential; you
cannot list, disable, or delete a user. The result: you can build up a workspace but never tear anything
down, and you cannot administer people. This scope supplies the missing destructive verbs and the user
lifecycle, holding the workspace wall on every one.

## Goals

- **Workspace lifecycle completed** — `workspace.rename` and `workspace.delete` (soft-delete:
  `Disabled`/`Archived`, then a guarded hard-delete) beside the existing `create`/`list`. Deleting a
  workspace tombstones it in the reserved directory and makes all its data unreachable; hard-delete is a
  separate, explicitly-confirmed, capability-gated step (data destruction is irreversible).
- **User record CRUD over the dev store** — `user.create` / `user.list` / `user.disable` / `user.enable` /
  `user.delete`, backed by the existing dev credential store (no password DB / OIDC — that stays the
  pluggable later slice). A disabled user **cannot mint a session** (the login path checks the record);
  a deleted user is removed and their grants revoked. This is the genuinely-missing primitive: identity
  becomes administered data, not a side-effect of first login.
- **Team lifecycle completed** — `teams.delete` and `teams.rename` beside `authz-grants`'s
  `teams.create`/`add_member`. Deleting a team removes its membership edges and revokes team-inherited
  grants (live for Gate 3, on re-mint for Gate 2 — the freshness asymmetry from `authz-grants`).
- **Member remove** — `members.remove` (the missing destructive member verb), removing the `member` edge,
  gated by the same `teams.manage` admin cap. Idempotent; workspace-first.
- **Every destructive verb is capability-gated, workspace-first, idempotent, and tombstone-aware.** Delete
  of an absent entity is a success, never a cross-workspace reach; soft-delete before hard-delete wherever
  data loss is irreversible.
- **Expose the set over the gateway** (and Tauri) so the admin UI (`frontend/admin-console-scope.md`) can
  drive them — the same thin-route mirror the collaboration slice proved.

## Non-goals

- **No login/credential mechanism beyond the dev store.** Password hashing, OIDC, SSO, MFA stay a later,
  pluggable scope behind the same `verify`/login seam (the `collaboration-scope.md` and `authz-grants`
  non-goal, restated). `user.create` here seeds a dev credential; the real IdP slots in later.
- **No grant/role *model*.** The grant store, roles, team-as-authz-primitive, and the create/assign verbs
  are `authz-grants-scope.md`. This scope **consumes** that model for revocation-on-delete and adds the
  destructive/user-lifecycle verbs around it. Build order is flexible; the two compose.
- **No `org` tier above workspace** (README §7 defers it). All verbs are workspace-scoped; deleting a
  workspace is the top of the hierarchy.
- **No 4th gate.** Enforcement stays the three gates (`caps::check` + `visibility`). These verbs are
  ordinary capability-gated host services that *write* the records the gates read.
- **No cascading cross-workspace effects.** A user may exist in multiple workspaces (per-ws grants);
  `user.delete` is **workspace-scoped** by default — deleting the principal globally is a distinct,
  explicitly-flagged node-directory operation, not the default verb.
- **No audit-log subsystem.** Destructive verbs should be *recordable*, but a full audit/event-sourcing
  surface is a later scope; here, note the hook, don't build the system.

## Intent / approach

**Symmetric soft-delete, guarded hard-delete, workspace-first always.** Every destructive verb resolves
the workspace, runs its capability gate, then performs an **idempotent** record operation. Data-losing
deletes are **two-step**: a reversible `disable`/`archive` (the default the UI offers) and a separate,
explicitly-confirmed `delete --hard` that destroys data — so an admin never nukes a workspace with one
mis-click.

```
  admin UI / agent / Tauri ──► gateway/MCP verb ──► capability gate (workspace-first)
                                                          │
                              ┌───────────────────────────┼────────────────────────────┐
                              ▼                            ▼                             ▼
                    soft-delete (default)         revoke grants (authz)         hard-delete (guarded)
                    status → Disabled/Archived     remove member edges          destroy records + cred
                    reversible, instant            live Gate 3 / re-mint Gate 2   irreversible, confirmed
```

- **Workspace**: `rename` updates the directory record; `delete` soft (status `Archived` in the reserved
  `WORKSPACES_NS` directory, data left but unreachable) then optional hard (drop the namespace's data) —
  the hard step is its own verb + confirm.
- **User**: a new `user` record per `(ws, user)` (the dev store gains a durable record, not just a
  credential map). `create` seeds it; `disable` flips `active=false` so the **login path refuses to mint**;
  `delete` removes the record + revokes the user's grants (calls into the `authz-grants` revoke). Login is
  amended to check `active` — the one place this touches the session path.
- **Team**: `delete` removes the team's `member` edges + revokes its grants; `rename` updates the team
  record. Reuses `authz-grants`'s `team` records and the `teams.manage` cap.
- **Member**: `remove` deletes the `(user, team)` `member` edge — the S4 edge the `visibility` resolver
  reads — so resource access via that team drops **live** (Gate 3); inherited caps drop on re-mint (Gate 2).
- **Revocation reuse**: deletes that strip access call the **existing** `authz-grants` revoke + the live S4
  membership resolver — no new enforcement, just the destructive writers feeding them.
- **Gateway/Tauri mirror**: each verb gets a thin route reading the session token; the `http.ts` map gains
  the entries. Same four-file move as every collaboration surface.

**Rejected alternatives:**
- *One-step hard delete everywhere.* Rejected — workspace/user deletion is irreversible data loss; a single
  verb invites catastrophic mis-clicks and offers no undo window. Soft-delete-then-guarded-hard-delete is
  the safe default; the UI defaults to the reversible step.
- *Skip user records; keep login auto-seed.* Rejected — you cannot `disable` or `list` a principal that has
  no record. Identity must be administrable data; a durable `user` record is the minimum, and it's where
  the real IdP later attaches. The dev credential becomes one field on the record.
- *Fold these into `authz-grants`.* Rejected as a single scope — `authz-grants` is already large (the model
  + create/assign); the destructive/user-lifecycle half is a coherent, separately-shippable slice that
  *uses* that model. Two scopes, one composed surface.
- *Global user delete by default.* Rejected — a principal can hold grants in several workspaces; the
  default destructive verb must respect the wall (per-workspace). Global removal is a deliberate,
  separately-flagged node-directory action.

## How it fits the core

- **Tenancy / isolation:** every verb is workspace-first. A ws-B admin cannot rename/delete ws-A, cannot
  list/disable/delete ws-A users, cannot remove ws-A members or delete ws-A teams. Workspace delete is the
  *top* of the wall — a ws-B caller targeting ws-A's id deletes nothing. Tested over store + MCP with two
  real sessions (the collaboration-slice two-principal test, now exercising the destructive verbs).
- **Capabilities:** each verb is gated by an **admin** cap — `mcp:workspace.delete:call`,
  `mcp:user.disable:call`, `mcp:teams.manage:call` (reused for team/member destructive verbs),
  `mcp:user.manage:call`. Deny is opaque. Hard-delete requires an *additional* explicit gate/confirm token,
  so the destructive ceiling is higher than the reversible one. Authz administration stays capability-first.
- **Symmetric nodes:** config-free host services; gateway role + Tauri in-process, two transports over one
  verb set. No `if cloud {…}`. Soft/hard-delete behave identically on every node.
- **One datastore:** workspace directory records, the new `user` records, team/member edges — all SurrealDB,
  workspace-scoped (the workspace directory in its reserved namespace). No new store; the dev credential
  becomes a field on the `user` record rather than a separate map.
- **State vs motion:** all of this is **state**. A "user disabled / workspace archived" notice, if shown
  live, is ordinary motion the admin surface publishes — not part of the durable verb.
- **Stateless extensions:** N/A to the verbs themselves; but disabling/deleting a user correctly revokes
  the grants that gate extension tools — the blast-radius story stays consistent.
- **MCP is the contract:** every verb is an MCP tool the gateway, Tauri, an admin agent, and the UI call
  identically. The admin console is one caller.
- **Durability:** destructive writes are single transactional record operations. A *hard* workspace delete
  (dropping namespace data) is a guarded, must-succeed operation; if it ever fans out to external effects
  (e.g. notify), those ride the outbox — but the core delete is one tx.
- **Sync / authority:** the workspace directory and `user`/`team`/`member` records sync on the §6.8 path;
  a soft-delete (`status` flip) is an idempotent `(table,id)` upsert that replays cleanly. Hard-delete is
  hub-authoritative and tombstoned so a stale edge can't resurrect it. **The freshness asymmetry from
  `authz-grants` applies and is reinforced:** member-remove / team-delete drop Gate-3 access live but
  Gate-2 inherited caps only on re-mint — state it on every revoking verb.
- **Secrets:** the dev credential (a stored secret-ish field) is mediated by the host, never returned to a
  caller; `user.list` never exposes it. The token-signing key is unchanged.
- **One responsibility per file:** one verb per file — `host/src/workspaces/{rename,delete}.rs`,
  `host/src/users/{create,list,disable,delete}.rs`, `host/src/members/remove.rs`,
  `host/src/teams/{delete,rename}.rs`. One route per file in the gateway. No `utils.rs`.
- **SDK/WIT impact:** **none** — flagged. All host-internal records + gates; the WIT boundary is untouched.

## Example flow

1. **Admin lists users** in `acme` (`user.list`) → `alice (active)`, `bob (active)`, `carol (active)`.
2. Admin **disables `bob`** (`user.disable`) → his record flips `active=false`. Bob attempts to **log in**
   → the login path refuses to mint (no session). Existing tokens expire within TTL.
3. Admin **removes `bob` from `facilities`** (`members.remove`) → the `member` edge is gone; a doc shared
   only to `facilities` is **immediately** unreadable to any session he might still hold (Gate 3 live);
   his inherited `operator` caps would drop on re-mint (Gate 2) — but he can't re-mint (disabled).
4. Admin **deletes the `facilities` team** (`teams.delete`) → its remaining member edges and team grants
   are removed; members lose team-inherited access on next re-mint, and team-shared resources drop live.
5. Admin **archives the `pilot` workspace** (`workspace.delete`, soft) → it's hidden from `workspace.list`
   and its sessions can't be minted; data is retained, reversible by `workspace.rename`/un-archive.
6. Admin **hard-deletes `pilot`** (`workspace.delete --hard`, separate confirm + `mcp:workspace.delete:call`
   + the hard gate) → the namespace's data is destroyed; the directory tombstone remains so it can't
   resurrect via a stale sync.
7. A **ws-B admin** attempts any of the above against `acme` ids → opaque deny / empty list. The wall holds
   across every destructive verb.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny** — over the **real gateway route** and MCP: a non-admin is refused each verb
  (`workspace.delete`, `user.disable`/`delete`, `teams.delete`, `members.remove`); the **hard-delete**
  requires its *additional* gate (granting the soft cap is not enough). The UI surfaces `Denied`.
- **Workspace isolation** — a ws-B admin cannot rename/delete ws-A, cannot list/disable/delete ws-A users,
  remove ws-A members, or delete ws-A teams; targeting a ws-A id deletes/leaks nothing. Two real sessions,
  across **store + MCP**. This is the collaboration two-principal test extended to destruction.
- **Offline / sync** — soft-deletes (status flips) and member/team removals replay idempotently after an
  offline edit; a tombstoned hard-delete is **not** resurrected by a stale synced edge; a re-issued delete
  after reconnect does not error or double-act.

Plus this slice's cases:

- **Idempotency** — double `user.delete`, double `members.remove`, double `teams.delete`, double
  `workspace.delete` are each a no-op success; no cross-ws deletion; deleting an absent entity succeeds.
- **Disable bites the login path** — a `disabled` user cannot mint a session; `enable` restores it;
  `user.list` never returns the credential field.
- **Revocation correctness (the freshness asymmetry)** — `members.remove` drops a team-shared resource
  **live** (Gate 3) but inherited caps only on re-mint (Gate 2); `teams.delete` revokes the team's grants;
  `user.delete` revokes all the user's grants. Re-verify against the `authz-grants` revoke path.
- **Soft-before-hard** — `workspace.delete` (soft) hides + un-mints but retains data (reversible);
  `--hard` destroys + tombstones (irreversible); the hard step needs its own confirm + gate.
- **Gateway parity** — each verb through the real node over the gateway (mirror `gateway_test`); `http.ts`
  has an entry for each (no `unknown command`).
- **Vitest** — an admin-view test per destructive op on the fake (confirm dialogs for hard-delete);
  fakes match the route contracts 1:1.

## Risks & hard problems

- **Irreversible deletion with no undo.** Workspace/user hard-delete destroys data. The soft-delete default
  + a separate, explicitly-gated, explicitly-confirmed hard step is the mitigation — but the *whole point*
  is a safe destructive surface; a one-click nuke is a misfeature. Get the two-step + the extra gate right.
- **Tombstones vs sync resurrection.** A hard-deleted entity must not come back via a stale synced edge from
  an edge that hadn't seen the delete. Tombstone in the directory and have the sync apply respect it —
  coordinate with `sync-scope.md` (§6.8 idempotent apply). Easy to underestimate; a resurrected workspace
  is a real isolation hole.
- **The freshness asymmetry is a footgun.** "Removed Bob" feels instant but his **inherited caps live in
  his token until re-mint**. Every revoking verb must document this; the safest mitigation for a true
  lockout is `user.disable` (kills minting) *plus* short TTLs — surface that in the UI ("removed; full
  cap revocation on next sign-in / within TTL").
- **`user.delete` scope — per-workspace vs global.** A principal in three workspaces: the default verb is
  per-ws; global removal is a separate node-directory action. Mixing them risks either an incomplete delete
  (grants left in ws-B) or an over-broad one (nuked everywhere from a ws-A admin). Keep the default scoped
  and the global path explicit and higher-gated.
- **Login-path coupling.** `disable` only bites if the login path checks `active`. That's a change to the
  session mint path (`collaboration` slice's `login`/credentials); it must be made and tested, or disable
  is theater. Single, well-tested touch-point.
- **Build-order coupling with `authz-grants`.** Revocation-on-delete calls the grant revoke; team/member
  verbs use the `team`/`member` records. If this ships before `authz-grants`, stub the revoke seam and the
  team record minimally, or sequence `authz-grants` first. Name the dependency; don't duplicate the model.
- **Cap-grammar additions.** New caps (`workspace.delete`, `user.manage`/`user.disable`, the hard-delete
  gate) extend the grammar — confirm they fit the existing `mcp:<surface>.<verb>:call` shape (they do) and
  seed them into the built-in admin role.

## Open questions

- **Hard-delete confirmation mechanism** — a second capability (`mcp:workspace.purge:call`) vs a typed
  confirm token in the request vs both. Lean: a distinct purge cap **and** a confirm token, so neither a
  stray grant nor a stray click suffices.
- **User record shape and where the dev credential lives** — a `user` record `(ws, user, active, role,
  cred_ref)` with the credential mediated separately, vs the credential inline. Lean: `cred_ref` to a
  mediated store so `user.list` can never leak it and the real IdP attaches at `cred_ref`.
- **Per-workspace vs global user identity** — is `user:alice` the same principal across workspaces (one
  global identity, per-ws grants) or distinct per ws? Lean: one global principal id, per-workspace `user`
  records + grants; global delete is the explicit node-directory action.
- **Soft-delete retention/GC** — how long an archived workspace's data is retained before GC, and whether
  GC is automatic or an admin action. Lean: retain until explicit hard-delete; GC is a later store
  follow-up, not automatic.
- **Does `teams.delete` block if the team still has members,** or cascade-remove them? Lean: cascade-remove
  the edges + revoke grants in one tx (idempotent), with the UI showing the member count before confirm.
- **Audit hook** — do destructive verbs write an audit record now (even if no audit UI)? Lean: emit a
  best-effort audit motion + leave a `deleted_by/at` field on the tombstone; the full audit surface is a
  later scope.

## Related

- `scope/auth-caps/authz-grants-scope.md` — the grant/role/team **model** + create/assign verbs this
  completes with the destructive/user-lifecycle half; the **freshness asymmetry** is defined there.
- `scope/auth-caps/auth-caps-scope.md` — the three gates + token/grammar these verbs feed and extend.
- `scope/auth-caps/edge-trust-scope.md` — token-on-the-bus; relevant to how fast a revoked/disabled
  principal is rejected across nodes (the staleness window).
- `scope/frontend/admin-console-scope.md` — the admin UI that drives every verb here (with confirm dialogs
  for the destructive/hard-delete steps).
- `scope/frontend/collaboration-scope.md` — the S9 session/login path `user.disable` amends, and the
  two-principal isolation test these destructive verbs extend.
- `scope/tenancy/tenancy-scope.md` — the workspace wall every verb holds; workspace delete is its top.
- `scope/sync/sync-scope.md` — §6.8 idempotent apply + tombstones the hard-delete must respect.
- README **§6.6** (identity/auth/caps), **§7** (tenancy / the workspace wall), **§3.5** (the capability
  chokepoint).
</content>
</invoke>
