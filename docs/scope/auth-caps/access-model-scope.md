# Auth-caps scope — the access model (team-as-unit + dashboard dependency closure)

Status: scope (the ask). Promotes to `doc-site/content/public/auth-caps/` once shipped.

"Assign a user to a dashboard" is not a primitive and does not make the dashboard *work*. A live
session proved it: `user:bob` was given the dashboard record (shared to his team) **and held every
capability**, yet his page still rendered two broken cells — one referenced a **private panel**
never shared to him (`panel:aidan` → 403), the other queried a **datasource that does not exist**
(`demo-buildings` → 403, broken even for the admin who authored it). A dashboard is a **composite
with a transitive dependency closure** — panels, datasources, the query verbs, the per-endpoint
connect caps, and any bound variables — and access is real only when the *whole closure* resolves
for the assignee. This scope defines the access model that makes assignment trustworthy: **the team
is the unit of access** (a person is assigned by joining a team that carries a role + shared
resources + a nav), and **a dashboard-access preflight** computes the dependency closure and reports
exactly what is missing (unshared panel, absent datasource, missing cap, unbound required variable)
so "assigned" provably means "renders."

## Goals

- **One assignment primitive: team membership.** Adding a person to a team resolves all three access
  layers for them at once — (1) capabilities via the team's **role**, (2) resource visibility via
  what's **shared** to the team, (3) the menu via the **nav** shared to the team. No per-user,
  per-dashboard grant; the wall stays the team/workspace (rule 6).
- **A dashboard-access preflight verb** — `dashboard.access_check` — that, given a dashboard and a
  subject (or team), walks the cell dependency closure and returns a **per-dependency verdict**:
  the dashboard record, every referenced panel, every cell datasource + the `net:` endpoint it
  needs, the `viz.query`/`federation.query`/`store.query` verb caps, and every `required` variable.
  Green means the page will render; anything red names the exact missing grant/share/record.
- **Assignment surfaces the closure, never silently widens it.** When you share a dashboard (or add
  it to a team's nav), the tooling *offers* to share the closure to the same team and warns on gaps
  — a guided, explicit step. It never auto-grants a capability the assigner doesn't hold (no-widening,
  `caller ∩ admin_approved` stays intact).
- **The nav is a pure lens for WIDENING (never grants), but gates reach for NARROWING.** A menu entry
  never carries a cap and never *widens* reach — the fix for a broken assignment is to make the team's
  role hold the caps the entry points at, not to grant from the entry. **Update (`nav-reach-scope.md`,
  shipped):** a *curated* nav now also *gates* reach in the narrowing direction — it is the allow-list
  of reachable core surfaces (`reach:<surface>:view`, derived from the resolved nav at login, enforced
  by the dedicated `GET /surface/{s}` route). One page in the nav ⇒ that is the only page reachable
  (read included); a *fallback* nav (no curated menu) reaches all (`reach:*:view`, so a default
  member/admin is never locked out). This still never widens: reach is only ever emitted for a surface
  the resolver already kept, so the nav can subtract reachable surfaces but never add one.

## Non-goals

- **Redefining the role/cap catalog** — that's `authz-grants-scope.md`; and the prerequisite that
  "member" stops meaning "admin" is `login-hardening-scope.md` (until caps are role-scoped, layer 1
  is always open and this model can't gate anything). This scope *consumes* those, doesn't re-author.
- **Per-user (non-team) sharing** — deliberately excluded; access is team-scoped. A one-off "just
  this person" is a one-member team, not a new grant shape.
- **Auto-creating missing datasources** — the preflight *reports* an absent/broken source; wiring the
  datasource + its secret is the datasources surface (`datasources-scope.md`), not this scope.
- **Row-level / query-result redaction** — whether a member should see *all rows* a query returns is
  a datasource-authority concern, not dashboard assignment.

## Intent / approach

Access already has the right three layers (capability, gate-3 visibility, nav lens); the gap is that
nothing composes them or verifies the **dashboard dependency closure**. Two additions, no new wall:

1. **`dashboard.access_check` (read-only preflight).** Load the dashboard, enumerate its cells, and
   for each cell collect its dependencies: `panelRef` → a `panel:<id>` (gate-3 visibility check
   against the subject), each `sources[].tool` + `datasource.uid` → (a) the verb cap
   (`mcp:<tool>:call`), (b) the datasource record must exist and be readable, (c) the datasource's
   `net:tls:<host>:<port>:connect` endpoint cap, and each `variables[].required` → must be bindable.
   Resolve each against the subject's effective caps (`resolve_caps`) + the gate-3 shares, and return
   a structured report `{dep, kind, ok, missing_cap?/missing_share?/reason}`. It **grants nothing** —
   it is the "will it work?" answer the session had to discover by hand.
   *Alternative rejected:* letting the UI probe each cell live and show broken tiles — that's the
   status quo (bob's page 403'd cell-by-cell); it's after-the-fact, per-viewer, and gives the
   assigner no pre-assignment guarantee.

2. **Guided closure-share (assist, not automation).** `dashboard.share`/nav-assign tooling calls the
   preflight for the target team and, for each red dependency the assigner *can* satisfy, offers the
   matching action (share the panel to the team, add the source cap to the team's role, register the
   datasource). Each is an explicit confirmed step honoring no-widening — the assigner cannot share a
   panel they don't own or grant a cap they lack. Silent auto-grant is explicitly rejected: "assign a
   dashboard" quietly granting datasource + endpoint caps is how you get accidental data exposure.

Sequencing: **preflight first** (read-only, high value, unblocks "does it work?" immediately and is
the test oracle), **guided closure-share second** (UX on top of the preflight's verdicts).

## How it fits the core

- **Tenancy / isolation:** every dependency check is workspace-scoped; the closure walk never leaves
  the token's workspace. A datasource/panel/dashboard in another ws is structurally invisible (§7).
- **Capabilities:** the model's spine. Each cell source maps to a concrete cap
  (`mcp:viz.query:call`, `mcp:federation.query:call`, `mcp:store.query:call`) **plus** the datasource
  `net:tls:<host>:<port>:connect` endpoint cap (admin-approved, enforced pre-connect —
  `datasources-scope.md`). The preflight resolves these via the existing `resolve_caps`; the deny is
  the same opaque 403 the live routes already return — we surface it *before* assignment instead of
  after. Admin-gated (`mcp:dashboard.get:call` to read + the subject's caps to test); reporting
  another subject's effective caps rides `authz.resolve` (access-console).
- **Placement:** either — pure resolution over store records + the grant projection; symmetric, no
  `if cloud`. Endpoint connectivity itself is node-local (a datasource may be reachable from the
  cloud node but not an edge) — the preflight reports the *cap*; actual reachability is a datasource
  concern.
- **MCP surface (§6.1):** one **read** verb, `dashboard.access_check` (get-shape: given ids → a
  report). No write verbs of its own — the guided-share step *reuses* existing `panel.share` /
  `dashboard.share` / `grants.assign` / `datasource.add`, each already its own gated tool. No
  live-feed (a one-shot preflight, not a stream). No batch in v1, though "check this nav's whole
  template-group closure" is a natural batch follow-on (would return per-dashboard reports).
- **Data (SurrealDB):** read-only over `dashboard`, `panel`, `datasource`, the `share` edges, and the
  grant store. Writes nothing. State, not motion.
- **Bus (Zenoh):** N/A — request/response preflight.
- **Sync / authority:** reads cached identity/grant + workspace resources (§6.8); on an edge the
  endpoint-reachability caveat above applies and should be noted in the report, not hidden.
- **Secrets:** never touches secret *values*. A datasource's `secret_ref` is checked for *existence/
  grant*, never dereferenced — the hash/DSN stays mediated (§6.7). The report says "endpoint cap
  present / absent," never a credential.

## Example flow

Assigning the ops team a monitoring dashboard, and *knowing* it will render:

1. Admin builds `dashboard:site-health` (cells: a `panel:site-map` + a `federation.query` on
   datasource `plant-telemetry`, one `required` var `site`).
2. Admin shares it to `team:ops` (`dashboard.share`) and adds it to the ops nav.
3. **Preflight:** `dashboard.access_check {dashboard:"site-health", team:"team:ops"}` returns:
   - `dashboard:site-health` — ok (shared to team) ✓
   - `panel:site-map` — **missing_share** (private) ✗
   - `mcp:federation.query:call` — ok (ops role holds it) ✓
   - `datasource:plant-telemetry` — ok (exists, readable) ✓
   - `net:tls:10.0.0.5:5432:connect` — **missing_cap** (ops role lacks the endpoint) ✗
   - var `site` — bindable (tag facet present) ✓
4. Guided share offers: "share `panel:site-map` to team:ops?" and "grant the ops role
   `net:tls:10.0.0.5:5432:connect`?" — admin confirms each (both within the admin's own authority).
5. Re-run preflight → all green. Any ops member opening the page now gets every cell rendering — the
   record, the panel, and a live query — not a wall of 403 tiles.

Contrast today (the session's actual result): steps 3–5 don't exist; bob got the record + all caps
and *still* hit a private panel and a missing datasource, discovered only by the page breaking.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny-tests (required):** the preflight must report `missing_cap` for a subject lacking
  `federation.query`/`viz.query`/the `net:` endpoint, and — crucially — a **live** `federation.query`
  by that subject must actually 403 (the preflight's verdict matches the real route's behavior, not a
  parallel guess). This is the regression for the live finding (bob's broken cells).
- **Dependency-closure correctness (the headline):** a dashboard with (a) an unshared panel, (b) a
  missing datasource, (c) an unbound required variable each produce the exact red verdict; sharing the
  panel / registering the source / binding the var each flips it green; a fully-shared closure returns
  all-green **and** the subject can really render every cell end to end (real store + real gateway).
- **Workspace-isolation (required):** the preflight for a subject in ws-A over a dashboard whose
  datasource lives in ws-B reports the ws-B dep as unreachable/absent, never leaks its existence; a
  ws-B token cannot preflight a ws-A dashboard at all.
- **No-widening:** guided closure-share cannot share a panel the assigner doesn't own or grant a cap
  the assigner lacks — the offered action 403s exactly as a direct call would.
- **No mocks (CLAUDE §9):** all against the real gateway + SurrealDB, seeded with real dashboards,
  panels, datasources, shares, and grants. The federation datasource is a real spawned source per
  `datasources` test convention (`federation_sqlite_test.rs` pattern), not a fake.

## Risks & hard problems

- **The closure is deeper than two hops.** A `panelRef` panel has its *own* sources; a `template-group`
  nav entry fans one dashboard across many bound instances; a cell source may be a **saved query**
  (`query:<id>`) with its own caps, or a page-chain link to *another* dashboard. The walk must recurse
  (panel→its sources, saved-query→its verb+datasource) with cycle detection, and decide a depth bound.
  Under-scoping the closure gives false-green — the worst outcome (says "will work," doesn't).
- **Preflight/live divergence is the cardinal sin.** If the preflight resolves caps differently from
  the actual route (e.g. misses the `.catalog`/`.pin`-style wildcard gaps that already bit
  `credentials.rs`, or ignores the datasource owner-wall), it lies. It must call the *same*
  `resolve_caps` + gate-3 predicates the routes use, not reimplement them — one source of truth.
- **Endpoint reachability ≠ endpoint cap.** The `net:` cap can be present while the source is
  physically unreachable from *this* node (edge vs cloud). The report must distinguish "not permitted"
  from "not reachable here," or an admin will chase a phantom cap gap.
- **Variable bindability is fuzzy.** "Required var `site` is bindable" depends on tag facets / a query
  option-source resolving under the subject's caps — itself a mini-closure. v1 may check "a default or
  option-source exists" and defer full per-value validation.

## Open questions

- **Recursion depth / composition:** how far does the closure walk — panel→sources (1 hop) only, or
  fully transitive through saved queries and page-chains? Recommend: fully transitive with cycle
  detection, but ship v1 with dashboard + panels + direct cell sources + endpoint + required vars, and
  flag deeper hops as `unchecked` (never silently green).
- **Report shape:** flat list of `{dep, kind, ok, reason}` vs. a tree mirroring cell→dep nesting.
  Recommend flat + a `cell` field, so the UI can group but the test can assert simply.
- **Who can preflight for *another* subject/team?** Reporting a subject's effective caps is
  `authz.resolve` (admin-ish, provenance-bearing). Preflight-for-self should be member-level (a viewer
  asking "why is my tile broken?"); preflight-for-a-team is admin. Confirm the two cap levels.
- **Guided-share default:** opt-in per dependency (admin confirms each) vs. "share the whole closure
  to this team" one-click. Recommend per-dependency confirm in v1 (explicit > magic), with a
  "share all I can" convenience that still lists what it did.

## Related

- Sibling scope: `login-hardening-scope.md` (**prerequisite** — role-scoped caps, or layer 1 never
  gates), `authz-grants-scope.md` (the role/cap catalog the team role draws from),
  `access-console-scope.md` (resolved effective caps + provenance — the preflight is the
  dashboard-shaped view of the same resolution), `auth-caps-scope.md` (the grammar).
- Cross-surface: `../../skills/nav/SKILL.md` (the lens-never-grants rule this preserves),
  `../datasources/datasources-scope.md` (the `net:` endpoint cap + datasource records the closure
  checks), `../widgets/widget-platform-scope.md` + `panel` visibility (the panel gate-3 wall).
- README `§3.5` (capability-first, enforcement order), `§6.6` (RBAC), `§7` (the workspace wall),
  `§6.7` (secrets — why the closure checks a `secret_ref`'s grant, never its value).
- Skill: on ship, the implementing session writes/extends a skill for `dashboard.access_check` (the
  "will this page work for this team?" preflight is exactly an agent-/API-drivable surface) — likely a
  section in `skills/dashboard-mcp/SKILL.md` or a new `skills/dashboard-access/`. Not N/A.
- Source: `rust/crates/host/src/dashboard/`, `.../panel/visibility.rs`, `.../viz/query.rs`,
  `.../authz/` (`resolve_caps`), the `datasource` records + `net:` cap enforcement.
