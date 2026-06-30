# auth-caps — global identity / many-workspaces (session)

- Date: 2026-06-30
- Scope: ../../scope/auth-caps/global-identity-scope.md
- Stage: post-S10 platform (new core-auth-caps slice)
- Status: done

## Goal

Build the **global identity / many-workspaces (Slack) model** end to end: one global identity per
person in a reserved system namespace `_lb_identity`, a per-workspace `membership` roster, and a login
that resolves identity → memberships → the existing `(sub, ws, caps)` token. Every verb the scope names
(identity.create/get/list/workspaces, membership.add/remove/list) wired store → cap → MCP → gateway →
http.ts → UI. Exit gate: the workspace switcher + login resolve through `identity.workspaces`; the
Access console "People" tab reads `membership.list`; a zero-membership identity cannot mint; leaving a
workspace is a clean exit (live token refused).

## What changed

Backend (decisions §1–10 implemented verbatim):
- `lb_authz`: new raw `identity.rs` (`_lb_identity` namespace — `identity_create/get/list`,
  `Identity{sub,display_name?,created_ts}`) + `membership.rs` (per-workspace `membership:{sub}` =
  `{sub,joined_ts}`, tombstone on leave — `membership_add/remove/get/list/is_member`).
- `crates/host/src/identity/`: `create/get/list/workspaces` host verbs, gated
  `mcp:identity.manage:call`; `workspaces` is the hub-only bounded scan (reads the workspace directory,
  checks each ws's `membership` table for `sub`).
- `crates/host/src/membership/`: `add/remove/list` host verbs gated `mcp:members.manage:call`. `add`
  writes the row AND grants `role:member` (raw `grant_assign` — a system effect, not a caller
  widening). `remove` tombstones the row AND composes the shipped `revoke_subject` + `token_revoke_mark`
  (does not duplicate them). `list` returns **effective members** = membership rows ∪ legacy `user:*`
  rows (lazy migration #10), lazy-creating `identity:{sub}` on first touch.
- `workspace_create` gains the first-member bootstrap (#3): after registering the directory entry it
  auto-memberships the creator AND grants `role:workspace-admin`.
- Login (`routes/login.rs`) now resolves membership: a sub with an effective membership in the
  requested ws mints; an **empty** ws bootstraps the requester as `workspace-admin` (the dev-login
  stand-in for #3, preserving the auto-seed demo); a ws that has members but not this sub → `403` "not a
  member" (#4). Identity is lazy-created on first login.

Gateway + client + UI:
- New routes `routes/identity.rs` + `routes/membership.rs`; registered in `server.rs` + `routes/mod.rs`.
- `http.ts` cases: `identity_*`, `identity_workspaces`, `membership_*`.
- `member_caps()` + `admin-caps.ts` carry `mcp:identity.manage:call` + `mcp:members.manage:call`.
- People tab (`PeopleAdmin.tsx`) roster re-points from `user_list` → `membership.list` (decision #9);
  "New user" = `identity.create` + `membership.add`.
- Workspace switcher resolves through `identity.workspaces` (the workspaces this identity belongs to),
  with the directory as a fallback.

## Decisions & alternatives

- Chose **effective-members union (membership ∪ legacy user rows)** in `membership_list` so the lazy
  migration (#10) is honest and the existing console tests (which seed via `user_create`) stay green —
  "no access gained or lost" is pinned by a test. Alternative rejected: a big-bang rewrite of every
  `user:*` row into a membership (would churn every workspace's namespace at upgrade).
- Chose **login bootstrap-on-empty-workspace** as the dev-login realization of #3 (first-member
  bootstrap). The scope's strict reading ("zero memberships → no token") is preserved for any workspace
  that *already has members*: a provisioned identity cannot enter it without `membership.add`. The only
  auto-membership is the creator-of-an-empty-workspace rule (#3) — exactly as decided. Rejected:
  refusing the very first login to a fresh workspace (breaks the shipped demo + every existing test
  that signs into a unique fresh ws).
- Chose to grant `role:member` / `role:workspace-admin` via the **raw** `grant_assign` (system effect),
  not the gated `grants_assign` host verb — so the built-in role lands regardless of the caller's caps
  and is not blocked by the no-widening rule (a system join is not a user widening).
- `Subject::User(sub)` grant store is **unchanged** (#6): `sub` stays the human handle `user:ada`,
  globally unique; `display_name` is a separate non-unique field.

## Tests

Real infra only (no mocks, no fake backend); the only "fake" is the dev-login credential stand-in.
Green command output:

**`cargo test -p lb-authz`** — `3 passed`.
**`cargo test -p lb-host --test identity_membership_test`** — `7 passed`:
```
test denies_each_identity_membership_verb_without_its_grant ... ok
test ws_b_admin_cannot_see_or_touch_ws_a_membership ... ok
test one_identity_in_n_workspaces_resolves_n_memberships ... ok
test login_refuses_a_non_member_of_a_workspace_that_has_members ... ok
test membership_remove_revokes_grants_and_marks_token ... ok
test legacy_user_rows_are_implicit_memberships_no_access_change ... ok
test removed_membership_tombstone_replays_idempotently ... ok
test result: ok. 7 passed; 0 failed
```
**`cargo test -p lb-role-gateway --test identity_routes_test`** — `5 passed`:
```
test forged_identity_membership_call_by_non_admin_is_denied ... ok
test admin_creates_identity_adds_member_lists_roster ... ok
test login_bootstraps_empty_workspace_and_refuses_a_non_member ... ok
test membership_remove_is_a_clean_exit ... ok
test now_const_anchor ... ok
test result: ok. 5 passed; 0 failed
```
**`cargo build --workspace`** green; **`cargo test -p lb-authz -p lb-host -p lb-role-gateway`** all green.
**`pnpm test`** — `Test Files 24 passed, Tests 168 passed`.
**`pnpm test:gateway`** (slice) — `Membership.gateway.test` 4/4, `PeopleAdmin.gateway.test` 4/4 (the
re-pointed roster, seeded via the legacy `createUser` path → still lists bob through the lazy
migration), `DocView.gateway.test` 3/3; **`pnpm lint`** 0 errors (150 pre-existing legacy warnings);
**`tsc --noEmit`** clean; **`pnpm build`** green.

Mandatory categories covered: deny-per-verb (identity + membership, over the MCP bridge AND the
gateway), workspace isolation (ws-B cannot enumerate/mutate ws-A membership; `identity.workspaces`
from ws-B shows only ws-B), offline/sync (the removed-membership tombstone replays idempotently; a
hub-added membership reaches the read path), migration (legacy `user:*` rows → no access change),
login/zero-memberships, leave-is-a-clean-exit (live token refused on next verify).

**Known environmental flakes (NOT introduced this session):** the shared serial gateway has
pre-existing timing flakes — `SystemView` (bus peer-count, Zenoh discovery timing) fails **with and
without** this change and **in isolation**; a few timing-sensitive UI tests (App back/forward,
ChannelView post→render, channel.api history) flake intermittently in the full run but pass in
isolation. Verified by stashing the login change: the flake set pre-exists. The slice's own tests are
green and stable across repeated runs.

## Debugging

None opened for a slice-introduced break. (The `SystemView` bus flake and the intermittent shared-
gateway timing flakes were confirmed pre-existing by reverting the login change and re-running — not
attributed to this slice.)

## Public / scope updates

- Promoted to `public/auth-caps/auth-caps.md` (new **Global identity + membership** section) and
  `public/SCOPE.md` (new "Shipped (post-S10 — global identity)" section).
- Scope doc status flipped to **shipped** + cross-linked to this session and the public doc.
- The scope's "Decisions (resolved — no open questions)" §1–10 were implemented verbatim; no open
  questions remain and none were reopened.

## Dead ends / surprises

- The login membership gate is the load-bearing behavior change. It surfaced one **existing** test
  (`admin_routes_test::login_refuses_a_disabled_user_over_the_real_route`) that enshrined the OLD
  "anyone auto-seeds into any workspace" behavior; updated it to the new contract (disabled still
  bites; auto-seed is now first-login-to-an-empty-workspace). The `DocView` gateway tests needed the
  second/third users added as workspace members before sign-in (they previously relied on free
  login) — fixed by calling `membership.add` from the bootstrapped admin.
- Chose login-bootstrap-on-empty as the dev-login realization of decision #3 so the shipped demo +
  every `signInReal` into a fresh workspace keeps working, while still honoring #4 (a non-member
  cannot enter a workspace that already has members).

## Follow-ups

- Open questions pushed back to scope: none (all resolved).
- TODO future session: `bus.watch` "membership changed" motion for live multi-admin roster updates
  (scope N/A-now). `cred_ref` / OIDC credential slice (decision #7). Multi-hub identity sync (scope
  non-goal). Org/tenant tier above workspace (scope non-goal). Edge-role nodes mounting identity/
  membership write verbs is hub-only by decision #8; a config gate to refuse them on an edge-role
  node is a follow-up hardening (the verbs are currently mounted on the gateway/hub).
