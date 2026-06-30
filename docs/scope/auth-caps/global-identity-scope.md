# Auth-caps scope — global identity, many workspaces (the Slack model)

Status: **shipped** (2026-06-30). Promoted to `public/auth-caps/auth-caps.md`; session:
`../../sessions/auth-caps/global-identity-session.md`.

The README's tenancy model (`§7`) and the S0 auth decision (`auth-caps-scope.md`: `sub` = "global
identity") say **one person is a global identity who belongs to many workspaces and switches between
them** — the Slack pattern, named as the first collaboration target (README line 387). The current
code does not implement this: `user.create` / `listUsers` read+write a user row **inside a workspace's
SurrealDB namespace**, so "bob" in `acme` and "bob" in `globex` are two unrelated records; login
re-creates the user per workspace; the workspace switcher just re-issues a different token rather
than carrying one identity across. This scope closes that drift: make identity **global and
hub-authoritative**, link it to workspaces via a **membership record**, and resolve a login to the
list of workspaces a person belongs to — without weakening the workspace hard wall or changing the
token/cap grammar. It is the prerequisite that makes the shipped Access console, teams, and the
workspace switcher behave the way the README already says they do.

## Goals

- **One global identity per person.** A user is a record in a **system-level identity directory**
  (not a workspace namespace), authoritative on the hub (README §6.6). The token's `sub` already
  carries it (`auth-caps-scope.md`); this scope makes the directory + verbs behind it real.
- **Membership, not per-workspace user rows.** A person joins a workspace via a **`membership`
  record** (`global_sub → ws`) that lives in that workspace. "The users of acme" becomes "the
  members of acme" — a roster of global identities, not a fresh row each.
- **Login resolves to workspaces.** Authenticate the global identity ONCE → enumerate the
  workspaces it is a member of → pick one → mint the existing `(sub, ws, caps)` token. The workspace
  switcher becomes real: switching re-mints for another workspace the **same identity** belongs to.
- **Caps/grants stay workspace-scoped.** Your role and grants in `acme` are independent of `globex`
  (unchanged). Only *identity* is global; *authority* remains per-workspace. The three gates and the
  Access console operate unchanged on resolved per-workspace caps.
- **Invite/join flow.** An admin (or a self-join link) adds a global identity to a workspace; removal
  drops the membership (and triggers the shipped revoke seam). This is the "invited to another
  workspace" half of the Slack model.
- **Backwards-compatible migration.** Existing per-workspace `user:<name>` rows become implicit
  memberships for a global identity of the same name, so nothing breaks on upgrade.

## Non-goals

- **No org/tenant tier above workspace (yet).** This adds *global identity*, not a nesting level. An
  `org` above workspace (README §7's deferred enterprise-grid tier) stays out of scope — but the
  global directory this introduces is the natural home for it later, so the design must not preclude it.
- **No real IdP / SSO / OIDC.** The login seam is still a dev-login *behind the new identity
  resolution* (the README §6.6 "Auth: OIDC for human login" stays the later pluggable slice). The
  *identity* becomes real; the *credential check* stays dev for now.
- **No cross-hub federation.** A person's workspaces are assumed co-located on one hub (README §13
  open question). Multi-hub identity sync is a later slice.
- **No change to the capability grammar or token shape.** `sub`/`ws`/`role`/`caps` are unchanged;
  `sub` already *is* the global identity. This scope fills the store + verbs, not the contract.
- **No rework of the Access console.** The console already resolves/display effective caps; it simply
  re-points "People" from workspace-scoped rows to members (global identities) — a data-source swap,
  addressed in that slice's follow-up, not re-scoped here.

## Intent / approach

**Lift identity out of the workspace namespace into a system directory; link it back via a
membership record.** Today the wall is "workspace = SurrealDB namespace," and a user row lives inside
it. The change keeps that wall for *everything except identity*: a global identity lives in a reserved
**system namespace** (`_lb_identity`, sibling to the shipped `_lb_workflow_directory` reserved
namespace pattern), hub-writable only; each workspace gains a `membership` record
(`{global_sub, joined_ts}`) that is the thing the Access console, teams, and the resolver
treat as "a user of this workspace." A login authenticates the identity, lists memberships, and mints
the per-workspace token. Switching workspaces is a re-mint under the same `sub`.

This is the model the README already specifies (§7 "membership records reference the global
identity"; §6.6 "users are global identities … stored in a system directory on the hub"), so the scope
is **promotion of stated design to implementation**, not a new invention. It composes with everything
shipped: the token, the three gates, `resolve_caps`, grants/roles/teams, the Access console, and the
revoke/freshness seams all key on `sub` and `ws` exactly as today — they just stop assuming `sub` is
minted per-workspace.

**Rejected alternatives:**
- *Keep users workspace-scoped, "link" them by name.* Rejected — two workspaces' "bob" have no
  verified relationship; there is no authority for "same person," so switching/invite is a fiction.
  Identity must be a real, authoritative record.
- *A global user table inside every workspace (replicated).* Rejected — it re-breaches the wall (the
  global roster would sync into every namespace) and re-introduces the split-brain the wall exists to
  prevent. One system directory is the source of truth.
- *Make the workspace namespace the identity unit (no global layer).* Rejected — that is today's model
  and the thing this scope exists to fix; it cannot express "one person, many workspaces."
- *Add the `org` tier now.* Rejected as over-scoped (README §7 explicitly defers it). The system
  directory this introduces is org-tier-shaped, so it stays a future extension, not a dependency.

## How it fits the core

- **Tenancy / isolation:** the workspace hard wall is **unchanged** for all tenant data (grants,
  roles, teams, channels, docs, series). Identity is the ONE thing lifted to a system namespace, and
  it carries no tenant payload — only `{sub, display_name?, created_ts}`. A ws-B admin cannot read or
  mutate ws-A's membership (membership records live in ws-A's namespace); the global directory is
  read-only for resolution and writable only by the hub's login/admin path. The two-workspace
  isolation test extends to: ws-B cannot enumerate ws-A's members.
- **Capabilities:** new verbs are themselves admin-cap-gated: `identity.*` (system directory)
  `mcp:identity.manage:call`; `membership.*` (per-workspace) `mcp:members.manage:call` (the existing
  member-admin cap, extended). The deny path is real and tested. A forged cross-workspace
  add/remove is refused server-side (membership lives in the target workspace; ws comes from the
  token).
- **Placement:** **cloud-hub-authoritative for identity** (README §6.6), workspace-scoped for
  membership + grants. Edge nodes verify tokens offline (public key) and read cached identity; they
  do not mint identities or memberships (that is hub-only). No `if cloud` — the role a node runs
  decides whether the identity/membership write verbs are mounted.
- **MCP surface / API shape (§6.1):**
  - **Get/list** — `identity.get(sub)` / `identity.list` (system, admin-only); `membership.list(ws)`
    (the workspace's roster — the new "People" source); `identity.workspaces(sub)` (the workspaces a
    person belongs to — drives login + the switcher).
  - **CRUD** — `identity.create(sub)` (hub-only, provisions a global identity in no workspace);
    `membership.add(ws, sub)` / `membership.remove(ws, sub)` (the invite/join + leave). `membership.add`
    grants the `member` built-in role on join (decision #2; no `role_hint` arg); `create_workspace`
    auto-`membership.add`s the creator and grants `workspace-admin` (the first-member bootstrap,
    decision #3). `membership.remove` composes the shipped `revoke_subject` + the Access console's
    `revoke_tokens` lever for a clean exit.
  - **Live feed** — N/A now; the switcher refetches `identity.workspaces` on focus. An optional
    `bus.watch` "membership changed" is a follow-up.
  - **Batch** — N/A (membership is one-record-per-(ws,sub)).
- **Data (SurrealDB):** two new record families, no new tables in tenant namespaces:
  - `_lb_identity` (system namespace): `identity:{sub}` = `{sub, display_name?, created_ts}`.
  - `membership` (per-workspace table): `membership:{sub}` = `{sub, joined_ts}` — the
    thing that makes a global identity "a user of this workspace." (No `role_hint` — role is
    grant-driven; join grants `member` automatically. Decision #2.)
  The existing `grant`/`role`/`team` records are unchanged and still keyed by `Subject::User(sub)`
  (the global sub). Migration: each existing workspace-scoped `user:<name>` row → an
  `identity:{user:<name>}` (if absent) + a `membership:{user:<name>}` in that workspace.
- **Bus (Zenoh):** none required for v1. Membership changes are state. An optional "roster changed"
  motion for multi-admin liveness is a follow-up, not v1.
- **Sync / authority:** identity is hub-authoritative and **syncs as system data** (the same §6.8
  discipline, in the reserved namespace); membership is workspace-scoped state and syncs with the
  workspace. Edge operates on cached identity offline and re-syncs on reconnect — the same staleness
  reality `edge-trust` already owns.
- **Secrets:** none new. The token-signing key is unchanged; identity has no credential (dev-login)
  until the later OIDC slice.

## Example flow

1. **Provision** → admin `identity.create("user:ada")` (hub-only) creates the global identity in
   `_lb_identity`. Ada now exists as a person, **in no workspace** — she cannot mint a token yet
   (login requires ≥1 membership).
2. **Create a workspace (bootstrap)** → Ada `create_workspace("pilot")`: the system auto-`membership.add`s
   Ada to `pilot` AND grants her `role:workspace-admin`, so a fresh workspace always has exactly one
   admin and is never orphaned. (For an existing workspace, step 3 is the path.)
3. **Invite** → a `pilot` admin `membership.add("pilot", "user:ada", …)` writes a `membership:user:ada`
   record in `pilot`'s namespace (re-checks `members.manage` server-side; ws from the token) AND grants
   `role:member` (the default-on-join). Ada is now a member of pilot at the member tier.
4. **Login** → Ada authenticates (dev-login) → `identity.workspaces("user:ada")` (a hub-only scan of
   each workspace's `membership` table) returns `["pilot"]` → she picks `pilot` → the hub mints the
   existing token `(sub=user:ada, ws=pilot, caps=resolve_caps(...))`. Everything downstream (gates,
   Access console, teams) is unchanged.
5. **Second workspace** → a `globex` admin `membership.add("globex", "user:ada")`. Ada's switcher now
   shows both. She switches → re-mint `(user:ada, globex, …)` — same identity, different workspace,
   independent caps.
6. **Leave** → `membership.remove("pilot", "user:ada")` drops the record AND composes `revoke_subject`
   + `revoke_tokens` (the Access console lever) so her pilot access + live token end cleanly. She is
   still a member of globex; her global identity is untouched. If pilot was her last membership, her
   next login returns "not a member of any workspace."
7. **Carol (ws-B admin)** → `membership.list` in `globex` shows only globex's roster; she cannot
   enumerate or mutate pilot's membership (the wall). She can `identity.get` a global identity
   (read-only) but not provision one (hub-only).

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny** — over the real gateway: a non-admin is refused `identity.*` / `membership.*`;
  a forged cross-workspace `membership.add`/`.remove` is denied server-side (the UI gate is not the
  boundary); an edge-role node refuses identity provisioning (hub-only).
- **Workspace isolation** — ws-B's `membership.list` shows only ws-B; ws-B cannot add/remove in ws-A;
  `identity.workspaces(ws-A-sub)` from ws-B resolves only ws-B's membership. Two real sessions, across
  **gateway + store**.
- **Offline / sync** — a membership added on the hub reaches an edge after reconnect (idempotent
  replay); a removed membership is not resurrected by a stale synced edge (coordinate with
  `sync`/`edge-trust`, same tombstone discipline as `revoke_subject`).
- **Migration** — an upgraded workspace with legacy `user:<name>` rows resolves them to
  `(identity, membership)` correctly; no access is gained or lost across the migration.

Plus this slice's cases:

- **Identity ↔ membership correctness** — one global identity in N workspaces resolves to N
  memberships; switching mints tokens with the SAME `sub` and the correct per-workspace `caps`
  (`resolve_caps` per ws — independent, as today).
- **Login → workspaces** — after `identity.create` + memberships, `identity.workspaces` returns
  exactly the joined set; a freshly-created identity with no memberships returns empty (cannot pick a
  workspace → cannot mint).
- **Leave is a clean exit** — `membership.remove` drops the record, composes `revoke_subject` +
  `revoke_tokens`, and the subject's live token is refused on the next verify (mirror the Access
  console revoke test).
- **No mocks** — real store/gateway/hub, seeded via the real write path; the only fake permitted is a
  true external IdP behind one trait (none required for the dev-login v1).

## Risks & hard problems

- **The wall is "workspace = namespace"; identity must live outside it.** A system namespace
  (`_lb_identity`) is a new top-level thing the store must serve without breaching the per-workspace
  wall. Mitigation: mirror the shipped reserved-namespace pattern (`_lb_workflow_directory`); the
  identity namespace is hub-writable, resolution-read-only, and carries no tenant data. This is the
  org-tier-adjacent step README §7 defers — name it, do not pretend it is trivial.
- **Identity authority vs edge offline.** The hub is the authority; an edge that provisions a user
  while partitioned would split-brain. Mitigation: identity/membership writes are **hub-only**;
  edges verify tokens offline and cache identity, never mint it (consistent with README §6.6).
- **Backwards compat / migration.** Existing per-workspace `user:<name>` rows must become
  `(identity, membership)` with no access change. Mitigation: a one-shot migration treats each
  workspace's user rows as an implicit membership for a global identity of the same name; a test pins
  "no access gained or lost."
- **`Subject::User` is already the grant target.** Grants key on `Subject::User(sub)`; if `sub`
  becomes a global id, the grant store is unchanged (the sub string is the same). The risk is a
  display/roster conflation (the People tab listing workspace rows vs members). Mitigation: the
  People tab swaps to `membership.list`; grants stay keyed by the global sub.
- **Name uniqueness / display.** A global identity needs a globally-unique `sub`; display names may
  collide across workspaces. Mitigation: `sub` is the unique key; display name is per-identity, not
  per-workspace (workspace-local aliases are a follow-up).
- **Scope creep into the org tier.** "Global identity" invites "groups of workspaces." Mitigation:
  hold the line — identity + membership only; the system directory is org-tier-shaped but the org
  tier stays a non-goal.

## Decisions (resolved — no open questions)

These were the open questions; all are now decided so an implementing session proceeds without
re-deliberation. They cover the four gaps the peer review raised (membership resolution, default role
on join, first-member bootstrap, login/provisioning rule) plus the two retrofit-expensive choices
(opaque `sub`, global credential ref).

1. **Identity-store location → a reserved system namespace `_lb_identity`.** Not a "system workspace"
   (that would blur "workspace = tenant"). Mirror the shipped `_lb_workflow_directory` reserved-namespace
   pattern (`crates/host/src/workflow/directory.rs`); the reserved name is disallowed as a real
   workspace by operator convention. Hub-writable, resolution-read-only, carries no tenant data.
2. **Membership record shape → pure "is this identity in this workspace", no `role_hint`.** A
   `membership:{sub}` row = `{sub, joined_ts}` in the workspace namespace. Role is grant-driven exactly
   as today — on join the system grants the **`member`** built-in role (see #3 for the admin case), and a
   workspace-admin grants more. Keeping `role_hint` out means membership and grants stay one-shape each.
3. **Default role on join + first-member bootstrap → `member` by default; the workspace creator is the
   first `workspace-admin`.** `membership.add(ws, sub)` grants `role:member` to `Subject::User(sub)`.
   `create_workspace(ws, principal)` auto-`membership.add`s the creator AND upgrades them to
   `role:workspace-admin` (the built-in admin bundle) — so a brand-new workspace always has exactly one
   admin and is never orphaned. The first member is the creator; nobody else can be added until that
   admin (or a super-admin) does it.
4. **Login / provisioning rule → an identity with zero memberships cannot mint.** `identity.create`
   provisions a global identity in NO workspace. Login resolves `identity.workspaces(sub)`; the picker
   shows the joined set; **zero memberships = "you are not a member of any workspace"** and no token is
   issued. The only auto-membership is the creator-of-a-new-workspace rule (#3). Provisioning ≠ joining.
5. **Membership-index resolution → the per-workspace `membership` table IS the index; `identity.workspaces`
   is a hub-only scan.** One source of truth: the `membership` rows in each workspace namespace.
   `identity.workspaces(sub)` resolves by scanning each workspace's `membership` table on the hub
   (bounded — a hub hosts few workspaces; this is NOT a per-request hot path, it runs once at login +
   when the switcher opens). No denormalized reverse index in v1 (it would be a second source of truth
   and drift); add one only if a real hub proves the scan slow.
6. **Opaque `sub` → NO. Keep the human-handle `sub` (`user:ada`), globally unique.** Grants already key
   on `Subject::User(sub)` and every existing grant uses `user:<name>`; switching to opaque `user:<uuid>`
   would retrofit every grant row. Instead the system directory enforces **global uniqueness of `sub`**;
   `display_name` is a separate, non-unique, per-identity field. Rename (changing a `sub`) is an
   explicit non-goal — it would orphan grants on purpose; use display names for human-facing changes.
7. **Global credential ref → NOT in v1.** The global identity record is `{sub, display_name?, created_ts}`
   — no credential. Login stays the dev-login behind the new identity-resolution seam. A `cred_ref`
   field (for the OIDC/SSO slice) is **additive later** — it costs nothing to omit now because no
   credential exists today, so there is no retrofit; it lands with the README §6.6 "OIDC for human
   login" slice. This decision is what keeps the global-identity slice from ballooning into an IdP.
8. **Edge provisioning → identity/membership writes are hub-only; edges are strictly read-only on
   identity.** Edges verify tokens offline (public key) and cache identity; they never mint identities or
   memberships. This is the consistency boundary — a partitioned edge cannot split-brain a new user.
9. **Access console "People" tab → re-points in THIS slice's follow-up, not a separate scope.** The
   console swap (workspace-scoped `user_list` → `membership.list`) is low-risk and is the proving
   surface for the new verbs, so it ships with the identity slice, not after. (The rest of the console
   is untouched.)
10. **Migration → lazy, not a big-bang upgrade.** A legacy `user:<name>` row with no `membership` row is
    treated as an implicit membership (resolved on first touch), and `identity:{user:<name>}` is created
    idempotently on first resolution if absent. No access is gained or lost; a test pins that.

> **Reinforcement found during peer review:** `crates/host/src/users/model.rs:1-3` already documents
> "per-workspace user records, one global principal id" — so the global-sub / per-workspace-row split
> this scope formalizes is half-acknowledged in the code today. The migration is therefore a
> re-pointing of an existing intent, not a reversal.

## Related

- README **§7** (tenancy: workspace = tenant; members are global identities; user belongs to many
  workspaces; org tier deferred), **§6.6** (identity/auth/caps; global identity on the hub), line 387
  (Slack clone = first target).
- `scope/auth-caps/auth-caps-scope.md` — the S0 token/principal decision (`sub` = global identity);
  its non-goal "org tier above workspace" is the tension this scope resolves by adding *identity*, not
  a tier.
- `scope/auth-caps/admin-crud-scope.md` — the destructive verbs (`user.create`/`delete`/`disable`)
  this scope refactors from "workspace-scoped user row" to "global identity + membership."
- `scope/auth-caps/access-console-scope.md` — the console whose "People" tab and `revoke_tokens`
  lever compose with membership.remove; its non-goal "No org/tenant-above-workspace" is consistent
  with this scope (identity, not a tier).
- `scope/auth-caps/edge-trust-scope.md` — the token-on-the-bus/offline-verify path; identity is
  hub-authoritative, edge verifies offline.
- `vision/0004-consumer-iot.md` — names the "global identity belongs to many workspaces" model and
  surfaces the org-tier gap this scope deliberately leaves open.
- `sessions/auth-caps/access-console-session.md` — where the workspace-scoped-vs-global divergence was
  confirmed and this scope was opened.
