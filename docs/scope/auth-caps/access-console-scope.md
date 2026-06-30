# Auth-caps scope — the Access console: a guided, access-first admin surface

Status: scope (the ask). Promotes to `public/auth-caps/` once shipped.

The admin verbs (workspaces · users · teams · members · roles · grants · api keys) and a unified
`/admin` console already exist and are cap-gated — but the console is a **flat five-tab directory**,
not an **access management tool**. You manage *records* (a user row, a team row) instead of
*access* ("who can do what, and how do I change it safely"). The capability model is exact; the
surface hides that exactness behind raw `mcp:…:call` strings in an "advanced" drawer, never shows a
subject's **resolved effective access**, has no way to act on the **freshness asymmetry** (a revoked
cap lingers in a live token until re-mint), and can't delete a role. This scope turns the console
into the **Access console**: one place where a sysadmin *sees* the access graph, *changes* it through
guided flows, and *understands the consequence and timing* of every change — closing the backend
gaps that block it (`roles.delete`, resolved effective caps **with provenance** over the shipped
resolver, and a **live-token** revoke lever on top of the shipped `revoke_subject` grant-revoke) and
rebuilding the UX access-first.

It is an **evolution of the existing `features/admin/` console and its verbs**, not a new page. A new
parallel "Access" page is explicitly rejected (it would recreate exactly the `/members`-vs-`/admin`
duplication that was just retired).

## Goals

- **Access-first information architecture.** The console's landing is an **access overview** of the
  workspace — people, teams, roles, and keys framed by *what they can do*, not by record fields.
  Browse "who can call `hvac.setpoint`" as easily as "what can bob do." The five entity tabs become
  views *into* the access graph, reached from the overview and from each other (person → their teams
  → a team's role → the role's caps), never dead-ends.
- **Effective caps, resolved and explained.** Every subject detail (person · team · key) shows the
  **union-resolved effective capability set** (`direct ∪ role ∪ team-inherited`, ∩ workspace), each
  cap tagged with **where it came from** (direct grant / role `r` / inherited via team `t`) — so an
  admin sees not just *that* bob can do X but *why*, and knows which grant to edit to change it.
  This **extends the shipped `resolve_subject_caps` / `resolve_caps` fold** (`crates/authz/resolve.rs`)
  with a provenance-tagging wrapper — it is not a new resolver. The wrapper runs the same union the
  session mint computes, so the displayed set and the enforced set cannot drift. Exposed over the
  gateway as an admin-only read.
- **Guided grant flows, not raw strings.** Granting a capability is a **picker over the real
  capability catalog** — grouped by surface (`mcp` tools / `store` / `bus` / `secret`), filtered to
  the no-widening set the admin holds, with human labels — not a free-text `mcp:…:call` field. The
  raw-string "advanced" drawer stays for power-users, but it is no longer the only path. Reuses the
  shipped `tools.catalog` verb (already capability-filtered) as the source of truth.
- **Act on the freshness asymmetry.** A destructive/revoking action surfaces the **timing** inline
  ("resource access drops now; cached caps drop on next sign-in / within TTL") AND offers a real
  lever: **kill the subject's live tokens** so an admin is not left praying at a TTL. The grant-revoke
  half already exists (`revoke_subject` — tombstones every grant a subject holds, idempotent,
  sync-safe, already wired into `users/delete`, `teams/delete`, `apikey/revoke`); it bites on the
  **next re-mint**. The genuinely-missing piece is the **live-token** half: a token-revocation marker
  the verify path checks so the *current* token is refused on the next request. This scope adds that
  live-token kill on top of the existing grant-revoke seam — it is not a greenfield revoke.
- **Close the `roles.delete` gap.** A role can be defined and assigned but not deleted; add the verb
  + the cascade (un-assign from every subject, idempotent, consequence shown).
- **Onboarding & honesty by default.** Empty states teach the model ("no teams yet — a team is how a
  group inherits a role's caps"); a missing/absent verb degrades to a disabled control with a reason,
  never `unknown command`; loading is skeletons, not spinners. Every action states reversibility and
  blast radius before confirm.
- **Cap-gated per control; server is truth.** Reaffirmed, not changed: the UI shows only what the
  session can do; the gateway re-checks every verb. A forged call is denied server-side.

## Non-goals

- **No new enforcement gate.** The three gates (`caps::check` + `visibility`) are fixed. This fills
  Gate 2's *input* (resolved caps display) and adds an admin *lever* over token freshness — it does
  not add a check.
- **No new IdP / credential mechanism.** Password / OIDC / SSO / MFA stay the later pluggable slice.
  `user.create` still seeds a dev credential. The session-invalidation lever operates on **existing**
  sessions/tokens, not on credential issuance.
- **No full audit-log UI.** S10's `audit/` scope owns the tamper-evident ledger. This scope shows
  best-effort *provenance* on a grant (last-changed-by/at if cheaply available) and routes destructive
  actions through the existing confirm path; the audit view is a later cross-link.
- **No org/tenant-above-workspace.** Everything is within the session's workspace (README §7).
- **No extension-page federation.** This manages access; it does not host extension UIs
  (`scope/extensions/ui-federation-scope.md` owns that).
- **No analytics/insights dashboard.** Access counts and "stale grants" hints are in scope as
  read-only overview tiles; trend graphs / reporting are not.

## Intent / approach

**Make the access graph the primary object; records are its projections.** Today the console is
five peer tabs each managing one record type. The redesign keeps those views but **re-centers them on
a shared access model**: a subject (user · team · key) has an **effective capability set** that is a
resolved projection of the grant store, and every view is a lens onto that projection. The new
landing is an **overview** (counts + the highest-leverage questions: who has admin-ish caps, which
subjects have direct grants bypassing roles, which keys are near expiry), and the existing tabs become
destination views you reach by drilling.

```
                       Access overview (landing)
                 people · teams · roles · keys at a glance
                 "who can do X"  ·  "what can bob do"
                              │ drill
   ┌──────────────────────────┼───────────────────────────┐
   ▼                          ▼                           ▼
 People detail            Teams detail               Roles / Keys
 ──────────────           ──────────────              ──────────────
 effective caps           members (inline)           cap-bundle editor
 (resolved, sourced)      team-inherited access      (picker, no widening)
 teams / roles            access editor              assignees of this role
 ─ guided grant picker ──── shared ──────────────── across all subjects
 ─ consequence + timing shown inline before every confirm ─
 ─ live-token revoke lever (on top of shipped revoke_subject) on revoking actions ─
```

- **Effective-caps resolver**: **extend** `crates/authz::resolve_subject_caps` / `resolve_caps`
  (shipped — the same union the session mint computes) with a **provenance-tagging wrapper** that
  returns `{ cap, source[] }[]` instead of `Vec<String>`. The wrapper folds the same grants/roles/
  teams the existing function does, recording which edge contributed each cap (direct grant / role
  expansion / team-inherited). One shared fold — the UI never re-derives caps client-side (it would
  drift from mint); provenance is a view over the one resolver, not a parallel implementation. Gated
  `mcp:authz.resolve:call`, admin-only, exposed over the gateway.
- **Guided picker**: the grant "advanced" raw-string field is augmented with a **catalog-driven
  picker** built on `tools.catalog` (shipped, already caller-cap-filtered) + the `store`/`bus`/
  `secret` surface grammar, filtered to the **no-widening set** (admin's own session caps). The
  picker offers human labels + grouping; selecting emits the canonical cap string to `grants.assign`.
  Raw-string entry stays as a power-user escape hatch.
- **The freshness lever**: the **grant-revoke** half already exists as `revoke_subject` (tombstones
  every grant a subject holds — next-re-mint). The **live-token** half is the new work: a
  `revoke_tokens(subject)` host verb (gated `mcp:authz.revoke-tokens:call`, admin-only) writes a
  token-revocation marker the verify path checks, so the subject's *current* token is refused on the
  next request — coordinated with `edge-trust-scope.md`'s token-on-the-bus verify. The revoking UI
  offers it as "Apply now (end their active sessions)" beside the honest default note, composing with
  `revoke_subject` (revoke grants + kill live tokens = full, immediate lockout). Multi-node: the
  marker syncs (§6.8) and a short worst-case TTL bounds the window — stated, not hidden.
- **`roles.delete`**: new verb (gated `mcp:roles.manage:call`), cascade-removes the `role:<name>`
  grant from every subject (idempotent), shows the affected-subject count before confirm. Built-ins
  are immutable and not deletable.
- **Overview tiles**: read-only aggregations over `list_users`/`list_teams`/`roles.list`/`apikey.list`
  + the new resolver — e.g. "direct-grant subjects (bypass roles): 3", "keys expiring <7d: 1",
  "subjects holding any admin cap: 2". Honest counts; no fabrication when a verb is absent.
- **Transport**: `lib/ipc/http.ts` gains the new route entries (`resolve`, `revoke_tokens`,
  `roles.delete`); the in-`http.ts` **transport shim** the existing admin surfaces use for Vitest
  mirrors them 1:1 (the sanctioned transport-fake pattern, **not** a parallel fake backend — every
  `*.gateway.test.tsx` still proves the route against the real gateway per CLAUDE §9). Same four-file
  move as every admin surface.

**Rejected alternatives:**
- *A new top-level "Access" page.* Rejected — it duplicates the cap-gated `/admin` console and
  recreates the `/members`-vs-`/admin` split just retired. One console, re-centered on access.
- *Per-call grant lookups instead of showing a cached projection.* Rejected for the hot path (the
  token is the cache) — but the **admin resolver is an explicit, admin-only, on-demand read**, not
  the hot path; that distinction is the whole reason this is safe.
- *A policy/ABAC DSL to make "who can do X" queryable.* Rejected — over-scoped. Effective-caps
  resolution + a catalog picker + overview tiles answer the real asks without a policy engine.
- *Hide raw cap strings entirely.* Rejected — they are the contract and power-users need them; they
  stop being the *only* path.

## How it fits the core

- **Tenancy / isolation:** every view operates within the session's workspace (the token's hard
  wall); `resolve_caps`, `revoke_tokens`, and `roles.delete` are workspace-scoped and a ws-B
  admin resolves/revokes/deletes nothing in ws-A. The two-principal isolation test extends to the
  new verbs.
- **Capabilities:** the new verbs are themselves admin-cap-gated (`mcp:authz.resolve:call`,
  `mcp:authz.revoke-tokens:call`, `mcp:roles.manage:call`); the deny path is real and tested. The
  guided picker is filtered to the **no-widening** set (admin's own caps), mirroring the server's
  `holds_cap` check — the UI cannot offer a grant the gateway will reject.
- **Placement:** `either` — core authz/host services compiled into every node; the console runs over
  gateway (browser) or in-process (Tauri), same verbs.
- **MCP surface / API shape (§6.1):**
  - **Get/list** — `resolve_caps(subject)` (single-subject resolved read **with provenance**, an
    extension of the shipped `resolve_subject_caps` fold, not a new resolver); the existing `list_*` /
    `roles.list` / `grants.list` / `apikey.list` power the overview.
  - **CRUD** — `roles.delete` (the one missing write verb this adds). No new create/update beyond it.
    (`revoke_tokens` below is a destructive action verb, not a record CRUD.)
  - **Live-token revoke** — `revoke_tokens(subject)` kills the subject's *current* token(s) on the
    next verify; composes with the shipped `revoke_subject` (grant-revoke, next-re-mint) for a full
    immediate lockout.
  - **Live feed** — N/A now; the overview refetches on focus and after each mutation. An optional
    `bus.watch` "access changed" hint is a named follow-up, not v1.
  - **Batch** — `roles.delete` cascade (un-assign from N subjects) is a **bounded, same-tx** operation
    (read assignees → revoke the `role:` grant in one `write_tx`); not a long batch, so it stays
    synchronous with the bound stated.
- **Data (SurrealDB):** no new tables. `resolve_caps` reads the existing `grant`/`role`/`team`/`member`
  records; `revoke_tokens` writes a revoke marker (a record the verify path already reads, per
  `edge-trust`); `roles.delete` removes the role record + its `role:<name>` grants in one tx. All
  state, workspace-scoped. (The shipped `revoke_subject` already tombstones grants — reused, not
  duplicated.)
- **Bus (Zenoh):** none required for v1. An optional "access changed" motion for multi-admin liveness
  is a follow-up; mutations are state.
- **Sync / authority:** the grant store is hub-authoritative and syncs on §6.8; the new verbs are
  idempotent `(table,id)` writes that replay cleanly. The **revoke_tokens marker** syncs so a stale
  edge can't keep a killed token alive beyond TTL — coordinate the tombstone with `sync-scope.md`
  (same discipline as the `admin-crud` hard-delete tombstone, and the same idempotent-replay shape
  the shipped `revoke_subject` already uses).
- **Secrets:** none new. `resolve_caps` never returns a credential; `apikey.list` keeps its
  hash/secret-redaction (shipped). The token-signing key is unchanged.

## Example flow

1. **Admin opens Admin** → the **Access overview** loads: tiles show "12 people · 4 teams · 3 roles ·
   2 keys", "direct-grant subjects (bypass roles): 3", "keys expiring <7d: 1", "subjects holding an
   admin cap: 2". Skeletons while loading; empty-state teaches if brand-new.
2. Admin clicks **"what can bob do"** → bob's **People detail** shows his **effective caps**, each
   tagged: `mcp:hvac.setpoint:call` *(via role `operator`, through team `facilities`)*,
   `store:series/hvac.*:read` *(direct)*. The admin sees *why* bob has each cap.
3. Admin wants to **grant alice** a tool → the **guided picker** lists `mcp` tools grouped by
   extension (from `tools.catalog`), filtered to the admin's own caps (no-widening); picking
   `hvac.setpoint` emits `grants.assign(user:alice, mcp:hvac.setpoint:call)`. No string typed.
4. Admin **removes bob from `facilities`** → the inline confirm states timing honestly ("team-shared
   resources unreadable now; bob's inherited caps drop on next sign-in") and offers **"Apply now —
   end bob's active sessions"** → `revoke_tokens(user:bob)` (the live-token kill), composing with the
   `revoke_subject` grant-revoke. Bob's next request is refused on the verify path.
5. Admin **deletes the `operator` role** → `roles.delete` shows "assigned to 4 subjects; un-assigns
   all" → confirm → the role + its `role:operator` grants are removed in one tx; built-in roles are
   not deletable.
6. **Carol (ws-B admin)** opens her console → sees only ws-B; `resolve_caps`/`revoke_tokens`/`roles.delete`
   against ws-A ids deny/empty. The wall holds across the new verbs.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny** — over the real gateway + MCP: a non-admin is refused `resolve_caps`,
  `revoke_tokens`, `roles.delete`; a forged direct call is denied server-side (the UI gate is
  not the boundary). The guided picker offers only caps ⊆ the admin's own (no-widening) — assert a
  cap the admin lacks is **not offered** and a forced `grants.assign` of it is server-rejected.
- **Workspace isolation** — a ws-B admin's `resolve_caps`/`revoke_tokens`/`roles.delete` against ws-A
  ids deny/resolve-empty/delete nothing; the overview shows only ws-B. Two real sessions, across
  **gateway + store**.
- **Offline / sync** — the revoke_tokens marker and `roles.delete` cascade replay idempotently after an
  offline edit; a tombstoned revoke marker is not resurrected by a stale synced edge (coordinate the
  tombstone test with `sync`/`edge-trust`).

Plus this slice's cases:

- **Effective-caps correctness** — `resolve_caps` returns exactly `union(direct, role, team-inherited)
  ∩ ws`, each cap tagged with the correct source(s); matches what `session.mint` would compute for the
  same subject (no resolver↔mint drift — a shared-code or cross-check test).
- **`roles.delete` cascade** — deletes the role + un-assigns from all subjects; idempotent on repeat;
  built-in roles rejected; affected-count shown matches reality.
- **Live-token revoke lever** — after `revoke_tokens(subject)`, the subject's prior token is refused on
  the next verify on the revoking node (and after sync, on a peer); a freshly-minted token still
  works; the worst-case multi-node window is bounded by TTL (asserted).
- **UX** — Vitest per view: overview tiles render honest counts (and empty/absent-verb states); the
  guided picker is filtered to no-widening and emits canonical cap strings; consequence + timing copy
  is accurate (content-asserted); the live-token revoke lever appears on revoking actions. Fakes mirror the
  three new routes 1:1. Real-gateway tests for `resolve`/`revoke_tokens`/`roles.delete` (mirror
  `admin_routes_test`).
- **No mocks** — tests run against the real store/bus/gateway seeded via the real write path; the
  only fake permitted is a true external IdP if one is stubbed behind one trait (none required here).

## Risks & hard problems

- **Resolver↔mint drift is a silent security hole.** If the displayed effective caps and the enforced
  (token) caps diverge, the admin *sees* one access set while the gate *enforces* another. Mitigation:
  provenance is a **tagging wrapper over the one shipped `resolve_subject_caps`/`resolve_caps` fold**
  — not a parallel implementation — so the resolver and the mint literally share code; plus a
  cross-check test that the wrapper's cap set equals `resolve_caps(...)` for the same subject. Do not
  re-implement resolution in the UI or in a second function.
- **The live-token lever can be weaponized / overused.** "End active sessions" is disruptive; an admin
  reflex-clicking it logs the subject out. Mitigation: it is a **secondary** action beside the honest
  default note, confirm-gated, and scoped to one subject (not "end all"). Document that the default
  revocation is already correct for most cases; the lever is for genuine lockout.
- **Multi-node revoke window.** A marker that hasn't synced to a peer lets a revoked token live
  until TTL. This is the same staleness reality `edge-trust` already owns — name it, bound it with
  TTL, and **do not claim instant global revocation**. The single-node case IS instant.
- **No-widening in the picker must match the server exactly.** If the picker offers a cap the server's
  `holds_cap` rejects, the admin hits a confusing deny after selecting. Mitigation: the picker's
  candidate set is the admin's **resolved** session caps (same source as the server check), with a
  test asserting parity; the raw-string escape hatch can still be server-rejected and must surface
  that cleanly (not `unknown command`).
- **Overview tile honesty.** Fabricated or stale counts mislead an admin about exposure. Mitigation:
  every tile is a real count from a real verb; absent verb → tile hidden with a reason, not a fake 0.
- **Scope creep into a policy engine.** "Who can do X" invites ABAC/queries. Hold the line: resolved
  caps + catalog picker + tiles. A query/search over caps is fine (client-side filter of resolved
  sets); a policy DSL is a non-goal.
- **Consequence-copy drift.** The timing/blast-radius text must match what the backend does (live vs
  re-mint, cascade counts). Keep strings beside the verbs and content-test them (the lesson from
  `admin-console-scope.md`).

## Open questions

> **All five resolved** in the build session `sessions/auth-caps/access-console-session.md` (2026-06-29) —
> shipped end to end. Kept here as the recorded decisions.

- **Live-token revoke mechanism** — ✅ RESOLVED → a per-`(ws, subject)` **tombstone RECORD** the
  verify path checks (`token_revoke:[ws, subject]`), NOT a nonce bump and NOT a global list. `revoke_tokens`
  **composes** with the shipped `revoke_subject` (marker + grant-revoke = full lockout). Single-node =
  instant; multi-node worst case bounded by TTL (stated in UI copy + a test comment, never "instant
  global"). See `crates/authz/src/token_revoke.rs` + the verify-path check in
  `role/gateway/src/session/authenticate.rs`.
- **How "who can do X" search works at scale** — ✅ RESOLVED → **client-side only for v1**: the UI
  filters/aggregates per-subject resolved sets (the overview tiles fold `authz.resolve` per subject).
  No `who_has` server verb built.
- **Overview tile set** — ✅ RESOLVED → ship exactly: People / Teams / Roles / Keys counts; direct-grant
  subjects (bypass roles); keys expiring <7d; subjects holding an admin cap. Provenance/last-changed
  tiles are OUT (see #5). A tile whose source verb/cap is absent is hidden with a reason, never a fake 0.
- **`roles.delete` vs "in use" guard** — ✅ RESOLVED → **cascade-un-assign** in ONE store transaction
  (`write_batch`): read assignees of `role:<name>`, tombstone that grant from each, delete the role.
  Built-ins (`super-admin`/`workspace-admin`/`member`) immutable — rejected with a clear `400`.
  Idempotent; affected count shown.
- **Provenance fields** — ✅ RESOLVED → **do NOT add** audit fields (`last_changed_by/at`) to grant
  records. The effective-caps view shows cap SOURCE only (direct / `role:r` / via `team:t`) from the
  resolver fold, not from new stored fields. Audit is owned by `scope/audit/`.

## Related

- `scope/auth-caps/authz-grants-scope.md` — the grant/role/team model + create/assign verbs whose
  resolved projection this console displays; the **freshness asymmetry** defined there.
- `scope/auth-caps/admin-crud-scope.md` — the destructive verbs this console drives and whose
  timing/leverage this surfaces.
- `scope/auth-caps/auth-caps-scope.md` — the three gates + grammar; `resolve_caps` (provenance)/`revoke_tokens` feed
  Gate 2's inputs/ageing, they add no gate.
- `scope/auth-caps/edge-trust-scope.md` — the token-on-the-bus verify path the **live-token revoke lever**
  hooks into; owns the multi-node revocation window.
- `scope/auth-caps/api-keys-scope.md` — keys as subjects in the same access graph (resolved caps +
  the picker apply identically).
- `scope/frontend/admin-console-scope.md` — the console shell + tabs this re-centers access-first;
  the "gated callers, never trusted deciders" rule this inherits.
- `scope/extensions/` (`tools.catalog`) — the capability catalog the guided picker reads.
- `scope/sync/sync-scope.md` + `scope/audit/audit-scope.md` — the idempotent-apply/tombstone
  discipline for the revoke_tokens marker, and the later audit view this lightly provenances.
- README **§6.6** (identity/auth/caps), **§3.5** (the chokepoint), **§7** (tenancy).
