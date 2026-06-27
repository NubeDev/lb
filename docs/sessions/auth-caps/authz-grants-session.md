# Auth-caps — authz: durable grants, roles & teams (session)

- Date: 2026-06-27
- Scope: ../../scope/auth-caps/authz-grants-scope.md
- Stage: S9+ (collaboration follow-up) — slice 1 of the admin-CRUD / lifecycle / console build. See STATUS.md.
- Status: done (backend slice; gateway routes + UI are slices 2 & 4)

## Goal
Add the **durable authorization model** the platform was missing: a per-workspace **grant store**,
**roles** (named cap bundles), and **teams** as a first-class primitive, plus the rule by which a
login session derives a token's caps from grants (`resolve_caps`). The 3-gate enforcement
(`lb_caps::check` + `lb_assets::visibility`) is **unchanged** — this fills Gate 2's *input* so caps
become administered data, not a hand-minted token. This slice also lands the two **seams** the
`admin-crud` slice (slice 2) consumes — `resolve_caps` (login projection) and `revoke_subject`
(revocation-on-delete) — so the destructive verbs call one model instead of duplicating it.

## What changed
- **New crate `lb-authz`** (`rust/crates/authz/`) — raw, workspace-namespaced store verbs, no
  authorization (the host service is the chokepoint, mirroring `lb-assets`):
  - `subject.rs` — `Subject ∈ {user, team, role}`, wire form `kind:name` (one flat column).
  - `grant.rs` — `grant(subject -> cap)` records; `grant_assign`/`grant_revoke`/`grant_list`/`granted`.
    Revoke is **tombstone-as-upsert** (not delete) so it replays idempotently under sync (§6.8),
    the same choice `lb_assets::unrelate` makes.
  - `role.rs` — `role(name -> caps[])` records; `role_define`/`role_caps`/`role_list`. Assigning a
    role is **not** a new verb — it is a `grant_assign` of the synthetic cap `role:<name>` (one
    assign/revoke path, not two); `resolve_caps` expands it.
  - `team.rs` — `team(team, name)` records (`team_create`/`team_list`); the `member` **edges** stay
    in `lb_assets` relations (the S4 edge the visibility resolver already reads).
  - `resolve.rs` — `resolve_caps(ws, user)` = `union(direct user grants, the user's roles' caps,
    for each team the user is in: the team's grants ∪ the team's roles' caps)`, deduped+sorted via a
    `BTreeSet` (deterministic token — testing §3). **This is the Gate-2 (cached) half of the
    freshness asymmetry**, documented on the fn.
  - `revoke.rs` — `revoke_subject(ws, subject)` tombstones every grant the subject holds (the revoke
    seam admin-crud calls on `user.delete`/`teams.delete`); returns the count for the consequence note.
- **New host service `authz`** (`rust/crates/host/src/authz/`) — the capability chokepoint over the
  raw crate, one verb per file:
  - `grants.rs` — `grants_assign`/`grants_revoke`/`grants_list`, gated `mcp:grants.assign:call` /
    `mcp:grants.list:call`.
  - `roles.rs` — `roles_define`/`roles_list`, gated `mcp:roles.define:call` / `mcp:roles.list:call`.
  - `teams.rs` — `teams_create`/`teams_list`, gated `mcp:teams.manage:call` / `mcp:teams.list:call`
    (`teams.manage` is the dedicated admin cap that retires the S4 doc-write stopgap).
  - `hold.rs` — `holds_cap(principal, ws, cap)`: the **no-widening** predicate. Parses the grant
    string into a `lb_caps::Request` and runs `lb_caps::matches` against the principal's own caps —
    so an admin can only grant/bundle caps they themselves hold (privilege-escalation guard).
  - `tool.rs` — `call_authz_tool`: the MCP bridge (`grants.*`/`roles.*`/`teams.*`) under the one
    contract, so the UI/agents/extensions call it like any wasm tool.
  - `mod.rs` re-exports the verbs + the two seams + the model types (`Role` aliased `AuthzRole` at
    the host boundary to avoid the node-`Role` collision, the same way `tags` aliases `Source`).
- Wired into `crates/host/src/lib.rs`; `lb-authz` added to the workspace + as a `lb-assets`-dep crate.

## Decisions (open questions resolved)
- **Role assignment = a grant of `role:<name>`**, not a separate verb. One mutation path; `resolve_caps`
  expands. Roles do not nest (a role's caps are plain strings), which also bounds expansion.
- **Where authz verbs live:** a dedicated host `authz` service (the scope's lean) — not tangled into
  `assets`.
- **No-widening enforced at define AND assign** of a plain cap (a `role:` grant is exempt — its caps
  were bounded at define time).
- **Team membership** stays the `lb_assets` `member` edge; `lb-authz` only adds the listable `team`
  record. `resolve_caps` walks `team_list` × `member` to find a user's teams (no reverse index needed).
- **Freshness asymmetry** is documented on `resolve_caps` and `revoke_subject`: Gate-2 caps are
  stale-until-remint; a true lockout needs `user.disable` (slice 2) + short TTL.

## Tests (green — `cargo test -p lb-host --test authz_test -p lb-authz`)
```
running 5 tests
test denies_each_admin_verb_without_its_grant ... ok
test no_widening_blocks_granting_a_cap_the_admin_lacks ... ok
test assign_and_revoke_are_idempotent_and_revoke_seam_strips_all ... ok
test ws_b_admin_cannot_see_or_touch_ws_a_authz ... ok
test resolve_unions_direct_role_and_team_inherited_caps ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 2 tests   (lb-authz unit: Subject round-trip + reject)
test result: ok. 2 passed; 0 failed
```
- **Capability deny (mandatory)** — `denies_each_admin_verb_without_its_grant`: a principal holding
  only `grants.list` is refused every other verb over the **real MCP bridge** (opaque `ToolError::Denied`).
- **Workspace isolation (mandatory)** — `ws_b_admin_cannot_see_or_touch_ws_a_authz`: a ws-B admin
  (full caps, wrong workspace) is denied list/assign against ws-A over MCP, and ws-A's grant does not
  leak into ws-B's `resolve_caps` (store layer). Two real sessions.
- **Grant resolution** — `resolve_unions_direct_role_and_team_inherited_caps`: caps = direct ∪
  team's role bundle; a non-member inherits nothing.
- **No-widening** — `no_widening_blocks_granting_a_cap_the_admin_lacks`: `AuthzError::Widen`.
- **Idempotency + revoke seam** — double-assign = one cap; `revoke_subject` strips all (returns count)
  and re-running is a no-op (idempotent replay, the offline/sync-friendly tombstone path).

`cargo build --workspace` green; `cargo fmt --check` clean; all new files ≤ ~237 lines (the test
file; sources ≤ 106) — FILE-LAYOUT respected; no `if cloud`; no SDK/WIT change.

## Follow-ups (next slices)
- **Slice 2 (admin-crud):** consume `resolve_caps` in the login path (collaboration `login`) and
  `revoke_subject` in `user.delete`/`teams.delete`; add `teams.delete`/`teams.rename`,
  `members.remove`, `user.*`, `workspace.rename`/`delete`+purge; gateway routes + `http.ts` + fakes.
- **Slice 4 (UI):** `GrantsAdmin` read + assign/revoke over `grants.*`/`roles.*`/`teams.*`.
- Built-in role seeding (super-admin/workspace-admin/member) + the login projection live in slice 2.
- Token TTL / revocation-on-the-bus staleness window: `edge-trust-scope.md` (separate).

## Related
- scope: ../../scope/auth-caps/authz-grants-scope.md (open questions resolved here)
- siblings it unblocks: ../../scope/auth-caps/admin-crud-scope.md, ../../scope/frontend/admin-console-scope.md
- model it feeds: ../../scope/auth-caps/auth-caps-scope.md (the 3 gates, unchanged)
