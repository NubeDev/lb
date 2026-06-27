# Session — admin console redesign (relationship-first + a real role editor)

Topic: `frontend` · Scope: [admin-console](../../scope/frontend/admin-console-scope.md) ·
Date: 2026-06-27

## The ask

The admin UI "looked like a chat window" and you couldn't tell "who belongs to who." Diagnosis:
every admin tab (`UsersAdmin`/`TeamsAdmin`/`MembersAdmin`/`GrantsAdmin`) reused the Slack channel
layout — a header, a flat list, and a **message-composer pinned to the bottom** to create records.
Worse, the entities were four disconnected flat lists; `Members` and `Grants` made you **type an
id** (`eng`, `user:bob`) to see anything, so relationships were undiscoverable. And **roles barely
existed**: the host had `roles.define`/`roles.list` (the latter already returning each role's caps),
but the UI threw the caps away, had **no role editor**, and made you type the raw synthetic
`role:<name>` string into a freeform cap box. You also (rightly) called out that demoing on **fake
data** is pointless — the running app must talk to the real gateway.

## What shipped

**Real backend wiring first (no fake-only feature).** The gateway only had `GET /admin/roles`; a
role editor literally couldn't talk to the backend. Added **`POST /admin/roles`** →
`define_role` → `lb_host::roles_define` (no-widening enforced server-side). Route registered in
`server.rs`; +1 Rust test (`admin_can_define_and_list_a_role_and_no_widening_is_enforced`) proving
define→list round-trip, the no-widening refusal (bundling a cap you don't hold → 403), and the
non-admin forged-call deny. **5/5 gateway admin-route tests green.**

**UI rebuilt around relationships, four tabs: People · Teams · Roles · Workspaces** (the old Users /
Members / Grants tabs folded in). The chat composer is gone everywhere — create is a header action
revealing an inline row.

- **People** — a selectable table (user · status · their teams) with a master-detail panel: status +
  enable/disable/delete, the **teams they belong to** (assembled from the real membership endpoints
  via the new `useDirectory` hook, never typed), and the shared `AccessEditor` (roles + advanced raw
  caps). This is the "who belongs to who" answer.
- **Teams** — table (team · member count) + detail with **inline members** (add/remove — the old
  separate Members tab, gone) and the team's `AccessEditor` (team-inherited access).
- **Roles** — the **real role editor**: a table of roles **with their cap count**, and a detail pane
  that builds/edits a role by **checking capabilities from a list** (no `role:` typing). The
  candidate caps are the admin's **own session caps** — exactly the no-widening set the server
  enforces, so the UI can't offer something the gateway will reject. Save = `roles.define` (replaces,
  so create and edit share one verb).
- **Workspaces** — restyled to the shared table/panel; lifecycle (archive/purge) unchanged.

**Shared pieces (FILE-LAYOUT, one responsibility each):** `AdminPanel` (the consistent header +
scroll body that replaces the chat frame), `AccessEditor` (one subject's roles + caps, reused by
People and Teams), `useDirectory` (users + teams + inverted memberships + their mutations, one
refresh source), `useSubjectGrants` (splits a subject's grants into roles vs raw caps),
`useRoles` (roles-with-caps + define). New api: `lib/admin/roles.api.ts` (`RoleView`, `listRoles` now
returns `RoleView[]`, `defineRole`); `http.ts` routes `roles_define`; the Vitest fake stores real
roles-with-caps.

**Deleted:** `UsersAdmin`, `MembersAdmin`, `GrantsAdmin` + their hooks (`useUsersAdmin`,
`useMembersAdmin`, `useGrantsAdmin`, `useTeamsAdmin`) and tests — folded into the four new surfaces.

## The boundary, unchanged

The gateway is still the only security boundary: every verb re-checks the cap server-side (the
forged-call deny is proven in Rust). The UI cap-gate (which tabs/controls show) is convenience. The
real-vs-fake transport split is unchanged — Tauri/gateway in the running app, the in-memory fake only
under Vitest (it is the test double, not demo data).

## Tests (green)

- **Rust:** `role/gateway` `admin_routes_test` — **5 passed** (incl. the new roles define/list +
  no-widening + non-admin deny).
- **UI:** full Vitest **57 passed** (20 files); `tsc --noEmit` clean; `pnpm build` clean. New/rewritten
  admin specs: `AdminView` (four-tab cap-gating), `PeopleAdmin` (who-belongs-to-who: a selected user's
  teams shown from membership; assign a named role from a dropdown; create via header action),
  `TeamsAdmin` (create → inline add member → delete cascade), `RolesAdmin` (build a role by checking
  caps; no-widening empty state).

## Follow-ups (not done)

- Effective-caps *resolution* in the People detail (currently shows direct roles + caps; a
  union-resolved "effective access" read would need a `resolve_caps` gateway verb).
- `roles.delete` (the host has define/list; delete is a tombstone follow-up).
- Tauri desktop command wiring for `roles_define` (the browser/gateway path is done).
