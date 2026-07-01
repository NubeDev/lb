---
name: auth-caps
description: >-
  The identity, capability, and workspace/tenant admin surface every other Lazybones skill builds on —
  log in for a token, bootstrap a workspace-admin, create/rename/archive workspaces, manage teams +
  members, and assign/revoke capabilities (grants). Use when a task says "log in / get a token",
  "create a workspace", "add a user/team/member", "grant a capability", "why is my call denied /
  forbidden", "set up permissions", or "call workspace/teams/members/grants verbs". Workspace isolation
  is checked FIRST, then capabilities within it (README §7 → §3.5); a denial is opaque. This is the
  chokepoint — nothing is reachable except through a host-mediated capability check.
---

# Identity, capabilities & workspaces (the security chokepoint)

Every Lazybones verb runs behind two structural walls, in order (README §7 then §3.5):

1. **Workspace isolation** — every key is scoped by workspace (= tenant); a ws-B caller physically
   cannot touch ws-A data. Checked first, structurally.
2. **Capability** — within the workspace, the caller must hold the capability the verb requires. A
   denial is **opaque** — "forbidden" and "absent" are indistinguishable (no existence signal).

The **workspace + principal come from the bearer token**, never from a request body. This skill is the
surface that issues tokens and administers the walls; every other skill starts with `login`.

## 1. Log in → a token

```bash
# dev login: who + which workspace. Logging into an EMPTY workspace bootstraps the caller as its
# workspace-admin (the first-principal bootstrap — how a brand-new tenant gets its first admin).
RESP=$(curl -s -X POST http://127.0.0.1:8080/login -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}')
TOKEN=$(echo "$RESP" | jq -r .token)
echo "$RESP" | jq '{principal, workspace, caps}'   # caps: what this token carries (UI-gating convenience)
```

Send `Authorization: Bearer $TOKEN` on every call. The reply's `caps` list is a **convenience for the
UI** (to hide dead buttons) — it is NEVER the security boundary; the gateway re-checks every verb
server-side. SSE streams take `?token=` instead (browser `EventSource` can't set a header).

## 2. The capability grammar

A capability is a colon-delimited string the verb names and the grant must cover:

- **`mcp:<tool>:call`** — permission to call an MCP verb, e.g. `mcp:dashboard.save:call`,
  `mcp:series.read:call`, `mcp:grants.assign:call`. This is the common one.
- **`bus:chan/{cid}:pub` / `:sub`** — publish/subscribe a channel (channels skill).
- **`net:tls:{host}:{port}:connect`** — an outbound endpoint (datasources skill; per-endpoint, admin-
  approved, enforced pre-connect).
- **`secret:{path}:get` / `:set`** — a secret (secrets skill).
- **`role:<name>`** — a named bundle of caps (assigned like a cap; its caps were bounded at definition).

Wildcards apply within a segment (e.g. `mcp:dashboard.*:call`, `secret:federation/*:get`).

## 3. Workspaces, teams, members, grants

Both a **dedicated REST admin surface** and the **`/mcp/call` bridge** expose these; both derive the
acting workspace + principal from the token. All are admin-gated and workspace-first.

| Action | REST route | MCP verb | Args / body |
|---|---|---|---|
| List workspaces | `GET /workspaces` | `workspace.list` | — |
| Create workspace | `POST /workspaces` | `workspace.create` | `{workspace}` |
| Rename / archive / purge | `POST /admin/workspaces/{ws}/{rename\|archive\|purge}` | `workspace.rename`/`.delete`/`.purge` | — |
| Which workspaces am I in | — | `identity.workspaces` | — |
| List / create teams | `GET\|POST /admin/teams` | `teams.list` / `teams.create` | `{team}` |
| Rename / delete team | `POST /admin/teams/{team}/rename`, `DELETE /admin/teams/{team}` | `teams.rename`/`teams.delete` | — |
| List / add members | `GET\|POST /admin/members` | `members.list` / `members.add` | `{sub, role?}` |
| Remove member | `DELETE /admin/members/{sub}` | — | — |
| Team membership | `GET\|POST /teams/{team}/members`, `DELETE /teams/{team}/members/{user}` | `membership.*` | — |
| List grants for a subject | `GET /admin/grants` | `grants.list` | `subject` |
| Assign a cap / role | `POST /admin/grants` | `grants.assign` | `{subject, cap}` |
| Revoke a cap | `POST /admin/grants/revoke` | `grants.revoke` | `{subject, cap}` |

A **subject** is `user:<name>`, `team:<name>`, or `key:<name>` (an API key). Grants to a `team:` apply
to its members.

```bash
# grant a member the ability to save dashboards, then confirm it
curl -s -X POST http://127.0.0.1:8080/admin/grants -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"subject":"user:bob","cap":"mcp:dashboard.save:call"}'
curl -s "http://127.0.0.1:8080/admin/grants?subject=user:bob" -H "authorization: Bearer $TOKEN"
```

## 4. The no-widening rule (the one that bites)

**`grants.assign` of a plain cap requires the assigner to HOLD that cap** — you cannot grant authority
you lack. (A `role:<name>` grant is exempt: the role's caps were already bounded when the role was
defined.) Assign/revoke are **idempotent** (store upserts) — re-granting or revoking-an-absent-grant is
a success. This is the same `caller ∩ grant` intersection that stops a rule or a saved query from
widening beyond its invoker.

## Gotchas

- **Everything comes from the token** — workspace, principal, and thus which walls apply. To act in
  another workspace, `login` into it; you can't name a foreign workspace in a body.
- **Empty-workspace login bootstraps an admin** — the first principal into a fresh workspace becomes
  its workspace-admin. In a real deployment, guard who can do this.
- **Denials are opaque** — a missing cap, a missing workspace, and a wrong-workspace resource all
  surface the same; if a call "vanishes", check the token's `caps` and that you're in the right ws.
- **The UI `caps` list is not the boundary** — the gateway re-checks server-side; hiding a control just
  avoids dead buttons.
- **You can't grant what you don't hold** — no-widening on `grants.assign` (plain caps).
- **Isolation is structural, not a check you can skip** — a ws-B token cannot enumerate or read ws-A
  keys even with a broad cap; the workspace prefix is applied by the store/bus layers.

## Related

- Scope: `docs/scope/auth-caps/` (identity, tokens, caps, the chokepoint), `docs/scope/tenancy/`,
  `docs/scope/workspace/`, `docs/scope/node-roles/`.
- The caps every other surface names: each skill's "capabilities" section
  (`docs/skills/*/SKILL.md`) — e.g. `datasources` (`net:*` + admin), `secrets` (`secret:*`),
  `channels-inbox-outbox` (`bus:chan/*`).
- README §3 (capability-first, workspace = the wall), §6 (identity/tokens), §7 (the hard wall).
- Source: `rust/crates/host/src/authz/`, `rust/role/gateway/src/routes/{login,admin_*}.rs`.
