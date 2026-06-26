# Auth-caps scope — authz: durable grants, roles, and teams (restricted user/team access)

Status: scope (the ask). Promotes to `public/auth-caps/` once shipped. A follow-up to the S0
`auth-caps-scope.md`, which fixed the *enforcement* (the three gates) but deferred the *source of
truth* for grants: "RBAC role hierarchy beyond the three named roles … scoped later (S3+)", and the S4
`add_member` note "a dedicated `teams.add_member` capability/role is the follow-up."

The three gates already work — workspace → capability → membership (`caps::check` + `assets/visibility`).
What's **missing is where the grants come from**: today a principal's caps live only in a **hand-minted
token**, teams are a side-effect of the doc-sharing flow, and there are exactly three built-in roles.
This scope adds the **durable authorization model** — a per-workspace **grant store**, **roles** that
bundle caps, **teams** as a first-class primitive, and the rule by which a **login session derives a
token's caps from grants** — so restricted user/team access is *administered as data*, not encoded by
whoever mints the token. The 3-gate enforcement is **unchanged**; this fills its inputs.

## Goals

- **A durable grant store.** Per workspace: `grant(subject -> caps)` where `subject` is a **user**, a
  **team**, or a **role**. The authoritative source of what Gate 2 checks — not the token alone.
- **Roles that bundle caps.** Beyond the three built-ins (super-admin | workspace-admin | member), a
  workspace can **define custom roles** (e.g. `operator`, `auditor`) as named cap bundles, and assign a
  role to a user or team. RBAC, workspace-scoped.
- **Teams as a first-class authz primitive.** Promote the S4 `member` edge into real `teams.*` verbs
  (create / add-member / remove-member), gated by an **admin capability** (not the S4 doc-write stopgap).
  A user **inherits** the caps granted to teams they belong to.
- **The session derives token caps from grants.** On login, the session mints a token whose
  `caps = ( direct user grants ∪ role caps ∪ team-inherited grants ) scoped to the workspace`. The token
  becomes a *cached projection* of the grant store, not the master record.
- **Resource-level restriction reuses S4.** "This specific doc/channel, for this team" stays the
  membership/visibility model (`private | shared-to-team | linked-to-channel`) — already built; this
  generalizes the team side and adds the admin surface to manage it.
- **Extension tools & pages gated by the same grants.** An installed extension's tools are
  `mcp:<ext>.<tool>:call` caps, bounded at install by `requested ∩ admin_approved` (existing). Granting an
  extension's tool to a user/team is an **ordinary grant**; its **pages** are gated callers — every page
  action re-runs the three gates, so a page can never exceed the user's grants.

## Non-goals

- **No login/credential mechanism.** *Where the session comes from* is `frontend/collaboration-scope.md`
  (S9, the minimal real session). This scope is *what caps that session mints* — the grant model behind it.
- **No cross-node token verification.** That's `auth-caps/edge-trust-scope.md` (token-on-the-bus). This
  scope produces the grants; that one verifies the resulting token across the wire.
- **No `org` tier above workspace** (README §7 defers it). Grants/roles/teams are all workspace-scoped.
- **No policy engine / ABAC.** Stay RBAC + resource grants + membership. Attribute/condition-based policy
  (time-of-day, IP, data-classification rules) is a later scope if ever needed.
- **No 4th gate.** The enforcement chain (`caps::check` + `visibility`) is fixed; this feeds Gates 2 and 3
  with real data and an admin surface — it does not add a new check.
- **No full extension-UI federation spec.** How module-federated pages mount is a dedicated
  `scope/extensions/` UI follow-up; here we only fix how their tools/pages **bind to grants**.

## Intent / approach

**Make Gate 2's input a record, not a hand-minted token.** The enforcement chokepoint already exists and
is tested; the weakness is that caps are conjured at mint time. So:

```
grant store (SurrealDB, per ws)        team graph (member edges)        roles (cap bundles)
        │                                      │                              │
        └──────────────┬───────────────────────┴──────────────────────────────┘
                       ▼  session.mint() computes the union, ∩ workspace
                 signed token { sub, ws, role, caps, exp }   ← a CACHED projection
                       ▼
                 caps::check (Gate 1 ws → Gate 2 cap) + visibility (Gate 3 membership)  ← UNCHANGED
```

- **Grant store**: `grant` records `(ws, subject, cap)` where `subject ∈ {user:…, team:…, role:…}`.
  `grants.assign` / `grants.revoke` manage them, gated by an admin cap.
- **Roles**: `role` records `(ws, name, caps[])`; `roles.define` / `roles.assign(subject, role)`. The three
  built-ins are seeded; custom roles are workspace data.
- **Teams**: `teams.create` / `teams.add_member` / `teams.remove_member` write the `member` edges the S4
  `visibility` resolver already reads — now gated by a real `teams.manage` cap, replacing the doc-write
  stopgap in `assets/add_member.rs`.
- **Session projection**: at login the session resolves `caps = union(user grants, roles' caps, teams'
  inherited grants)` for the workspace and mints the token. Token TTL bounds staleness (below).
- **Enforcement unchanged**: `caps::check` and `visibility::may_read_doc` are not touched — they already
  enforce exactly this; we are giving them a real data source and admin verbs.

**The freshness asymmetry (subtle but load-bearing): Gate 2 is stale-until-remint, Gate 3 is live.** A
**capability** change (grant/revoke a cap or role) only takes effect when the token is **re-minted** —
the token is the cached projection, bounded by its TTL. A **membership/visibility** change (add/remove a
team member, share/unshare a resource) is **live** — the S4 relation is re-resolved on every read, so it
bites on the *next call*. So "remove Bob from the team" does **two** things at different speeds: he loses
access to **resources shared to that team immediately** (Gate 3), but any **caps he inherited via the
team remain in his current token until it expires/re-mints** (Gate 2). Short TTLs keep that window small;
the example below shows both halves. Don't assume revocation is uniformly instant.

**Extension tools/pages** ride this directly: install fixes the extension's *ceiling* (`requested ∩
admin_approved`); a `grants.assign(team:facilities, mcp:hvac.setpoint:call)` lets that team use the tool;
the extension's page calls the tool and is gated like any caller. A page **cannot widen** beyond the
admin-approved install set — that intersection is the blast-radius rule (`ext-loader/grant.rs`).

**Rejected alternatives:**
- *Keep caps only in hand-minted tokens.* Rejected — unauditable, unmanageable, and the token can't be the
  master record (it's a bearer cache). The grant store is the source of truth.
- *A new enforcement gate for teams/roles.* Rejected — Gate 2/3 already enforce; adding a gate duplicates
  logic. Roles/teams resolve *into* caps + membership the existing gates read.
- *ABAC / a policy DSL now.* Rejected — over-scoped; RBAC + resource grants + membership covers the asks
  (restricted user/team access) without a policy engine.
- *Per-call grant lookups (no token cache).* Rejected for the hot path — the token is the cached
  projection; freshness is handled by TTL (+ optional revocation), not a DB hit per check.

## How it fits the core

- **Tenancy / isolation:** grants, roles, and teams are **all workspace-scoped records**; a ws-B admin
  cannot see or assign ws-A grants, and a ws-A team is invisible to ws-B. The wall holds at the data layer
  and at every gate.
- **Capabilities:** the **admin verbs are themselves capability-gated** (`grants.assign`, `roles.define`,
  `teams.manage` require an admin cap) — authz administration is capability-first, like everything. Deny
  is opaque.
- **Placement:** `either` — a core authz surface compiled into every node; grants sync like any record.
- **MCP surface:** `grants.assign/revoke/list`, `roles.define/assign/list`, `teams.create/add_member/
  remove_member/list` — MCP tools, called identically by the admin UI, extensions, and agents.
- **Data (SurrealDB):** `grant`, `role`, `team` + the existing `member` edges; resource grants reuse the
  S4 doc/channel relations. All state, workspace-scoped.
- **Bus (Zenoh):** none directly — authz is state. A "grants changed" hint, if wanted, is ordinary motion
  the admin surface publishes, not this crate's job.
- **Sync / authority:** grants/roles/teams are `(table, id)` records that sync on the §6.8 path; the
  token minted from them verifies **offline** with the issuer's public key. Hub-authoritative for the
  grant store; edges read a synced copy.
- **Secrets:** none — caps are not secrets (the token-signing key is, handled in `auth-caps`/secrets).

## Example flow

1. A workspace admin **creates a team** `facilities` and **defines a role** `operator` bundling
   `mcp:hvac.setpoint:call`, `store:series/hvac.*:read`.
2. Admin **assigns** `operator` to the `facilities` team, and **adds Bob** to `facilities`.
3. **Bob logs in** → the session resolves his caps = `operator`'s caps (inherited via the team) ∩ `acme`,
   and mints his token.
4. Bob **sees the HVAC extension's pages** (his grants include its tool) and can **call `hvac.setpoint`** —
   each page action re-runs the three gates server-side and passes.
5. **Alice**, not in `facilities`, calling the same tool gets `Denied::Capability` — opaque; and a page
   she loads that calls it shows a denied state.
6. Admin **removes Bob** from `facilities` → on his next token re-mint (or within TTL) the inherited caps
   are gone; a resource shared only to `facilities` becomes unreadable on the next call (the live S4
   relation).

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny** — a user without the granted cap/role is refused (over MCP); each **admin verb**
  (`grants.assign`, `roles.define`, `teams.add_member`) is refused without the admin cap; an extension
  **page action** beyond the user's grant is denied.
- **Workspace isolation** — a ws-B admin cannot assign, list, or see ws-A grants/roles/teams; a ws-A team
  is invisible to a ws-B caller; across **store + MCP**.
- **Offline / sync** — grants/roles/teams replay idempotently after offline edits; a token minted from the
  synced grant store verifies offline.

Plus this slice's cases:

- **Grant resolution** — the session mints `caps = union(user, role, team-inherited) ∩ ws`; a user in two
  teams gets the union; removing a team membership drops the inherited caps on re-mint.
- **Role bundles** — defining/assigning a custom role grants its caps; revoking it removes them.
- **Team-scoped resource access** — a doc shared to `facilities` is readable by a member, denied to a
  non-member, and **instantly unreadable** after the membership edge is revoked (re-verify the live S4
  relation through the new admin verbs).
- **Extension binding** — granting `mcp:<ext>.<tool>:call` to a team lets members call it; a page cannot
  exceed the `requested ∩ admin_approved` install ceiling.

## Risks & hard problems

- **Token staleness vs. revocation.** The token caches grants; a revoked grant is still honored until the
  token's `exp`. Mitigate with **short TTLs + re-mint** (lean) or an optional revocation check on the
  verify path (coordinate with `edge-trust`'s token-on-the-bus). State the staleness window explicitly.
- **The `add_member` stopgap migration.** Today `assets/add_member.rs` is gated on `store:doc/*:write` as
  an admin proxy; moving to a real `teams.manage` cap touches the S4 flow + its tests. Mechanical but must
  not break doc-sharing — keep the `member` edge shape identical, just change the gate.
- **Role / grant sprawl.** Custom roles + per-resource grants can explode into an unauditable mess. Needs
  a `grants.list`/`roles.list` audit surface and sane defaults; consider a per-workspace cap on custom
  roles. Easy to underestimate.
- **Don't add a 4th gate.** The temptation to encode role/team logic as a new check. Resist — roles/teams
  resolve *into* caps + membership the existing two gates read. A new gate is a bug.
- **Extension page over-grant.** A page must never let a user exceed the admin-approved install ceiling or
  their own grants. The intersection (`ext-loader/grant.rs`) + per-call gating must hold for the UI path
  exactly as for a direct tool call — test it.
- **Built-in vs custom role precedence.** super-admin/workspace-admin must not be redefinable into a
  privilege-escalation. Built-ins are seeded and immutable; custom roles can only grant caps the assigner
  themselves holds (no widening).
- **The blast-radius guarantee depends on a deferred scope — call it out, don't bury it.** "A page can
  never exceed the user's grants" holds **only if every page action re-runs the three gates server-side**.
  But *how extension pages mount and call* is deferred to the future `scope/extensions/` UI scope. So this
  security property is **asserted here while its enforcing runtime is out of scope** — and if that
  federation scope ever permits **client-trusted page logic** (a page that decides access in the browser),
  the guarantee **silently breaks**. **Owner: the `scope/extensions/` UI scope must inherit this as a hard
  requirement** — pages are gated callers only, never trusted deciders. Flag it there in writing; this is a
  cross-scope dependency, not a detail.

## Open questions

- **Token TTL + revocation strategy:** short-lived tokens (revocation-by-expiry) vs a revocation list /
  per-check freshness. Lean: short TTL + re-mint; align with `edge-trust`.
- **Team-inherited caps — computed at mint or resolved live?** Lean: computed at mint (the token is the
  cache); live resolution only for the membership/visibility Gate 3 (already live in S4).
- **Custom-role scope:** per-workspace role table (lean) vs a shared catalog. And: may a custom role grant
  only caps the definer holds? (Lean: yes — no widening.)
- **Resource-grant grammar:** the `store:doc/{id}:read` grammar already supports per-resource grants — do
  we expose a `grants.assign` for a specific resource, or keep per-resource as the S4 share/link relations?
  Lean: keep S4 relations for docs/channels; `grants.assign` for surface/tool caps.
- **Where teams/grants admin verbs live:** extend the existing `assets` host service, or a new `lb-authz`
  surface? Lean: a dedicated `authz` host surface so it isn't tangled with assets.
- **The extension-UI federation details** (how pages mount, design-token exposure, trusted vs iframe) —
  deferred to a dedicated `scope/extensions/` UI scope; this scope only fixes the *grant binding*.

## Related

- `scope/auth-caps/auth-caps-scope.md` — the three gates + token/grammar this feeds (closes its deferred
  "RBAC beyond three roles").
- `scope/auth-caps/edge-trust-scope.md` — verifies the token this produces across the wire (sibling).
- `scope/files/files-scope.md` + `scope/skills/skills-scope.md` — the S4 membership/visibility model
  (`add_member`, `visibility`, grant-gated skills) this generalizes and gives an admin surface.
- `scope/frontend/collaboration-scope.md` — the S9 session that *mints* the token from these grants, and
  the teams/members UI that calls these admin verbs.
- `scope/extensions/` + `scope/registry/` — the `requested ∩ admin_approved` install ceiling
  (`ext-loader/grant.rs`) extension tools/pages bind within.
- README **§6.6** (identity/auth/caps), **§6.13** (extension UIs / pages), **§7** (tenancy), **§3.5**
  (the capability chokepoint), **§11.5** (blast-radius / the install intersection).
