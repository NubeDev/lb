# Nav scope — the nav builder (user-/team-authored navigation over pages)

Status: scope (the ask). Promotes to `public/nav/nav.md` once shipped. Target stage:
**S9+ collaboration UI** (builds on the shipped S8 data plane and the S9 real-session shell,
the shipped `dashboard.*` asset + share model, `lb-authz` teams/roles/grants, and `lb-tags`).

We want a **nav builder**: an admin (or empowered member) composes a **navigation menu** for a
workspace — a named, ordered set of entries that each link to a **dashboard page** or a **system page**
(channels, rules, flows, datasources, …) or an **extension page** — assigns it to **teams** (or the
whole workspace, or keeps it personal), and each member sees the nav resolved to *their* effective
menu. Tags do double duty: a **dynamic entry** pulls in every dashboard matching a tag facet (tag a new
dashboard → it appears in the menu, no nav edit), and navs are themselves taggable for discovery. The
menu is a **lens over existing access, never a new grant path** — an entry the caller lacks the
capability for is simply stripped, and the gateway re-checks every verb regardless.

Today the sidebar is a **compile-time `SURFACES` array** in `ui/src/features/shell/NavRail.tsx`,
filtered display-only by `allowedSurfaces(caps)` (`ui/src/features/routing/allowed.ts`), plus
extension slots discovered via `ext.list`. There is **no user- or team-defined nav** anywhere. This
scope adds one.

---

## Goals

- **A `nav` asset** — a workspace-scoped, slug-identified, versioned record modeled exactly like
  `dashboard:{id}`: `title`, `owner`, `visibility (private | team | workspace)`, and an ordered
  `items[]`. Persisted in SurrealDB (state in the store, never `localStorage`). Shared to teams via the
  **same S4 `share` edge** dashboards already use (`dashboard/share.rs`).
- **Full nav CRUD as host MCP verbs** — `nav.get` / `nav.list` / `nav.save` (create+update) /
  `nav.delete` / `nav.share`, each capability-gated and workspace-first, wired end to end
  (store → cap → MCP → gateway route → `http.ts` → UI). The complete surface, not a read-only subset.
- **A resolver verb `nav.resolve`** — returns the caller's **effective** menu: their active nav picked
  (personal pick → team-shared → workspace default → built-in `SURFACES` fallback), with **tag-group
  entries expanded** (via `tags.find`) and every item the caller can't reach **already stripped**. The
  UI renders one payload and re-implements no filtering.
- **Four entry kinds** in `items[]`, plus one level of grouping:
  - `surface` — a core system page, referenced by its opaque surface key (`"channels"`, `"rules"`, …).
  - `dashboard` — a specific dashboard page, referenced by `dashboard:{id}`.
  - `ext` — an extension page, referenced by an **opaque** ext id (rule 10 — never branched on).
  - `tag-group` — a **dynamic** entry: `{ label, facets: [{key, value?}] }`, resolved at render time to
    the dashboards tagged with those facets.
  - `group` — `{ label, items: [...] }`, one nesting level, for sections/headers.
- **NavRail renders the resolved nav** — `ui/src/features/shell/NavRail.tsx` stops hardcoding and
  renders `nav.resolve` output, **falling back to today's `SURFACES`** when no nav exists (never a blank
  sidebar). Route gates (`CoreGate`) are untouched — the nav *hides*, it does not *block*; a deep link to
  a permitted-but-unlisted page still works.
- **A nav builder surface** — an admin UI (under the existing admin/access-console area) to pick items
  from the three real sources (`SURFACES`, `dashboard.list`, `ext.list`), add tag-group entries via a
  facet picker, drag to order/group, set visibility, and assign team shares.
- **A per-user active pick** — a tiny `nav_pref:[ws, user]` record (member-owned) for "which nav am I
  using" (and optional pinned/reordered favorites). **Not** in `lb-prefs` — its axis set is deliberately
  closed to formatting axes.

## Non-goals

- **No new authorization system.** The nav grants **nothing**. Access to a page is decided by the
  caller's existing caps and the target's own visibility (dashboard `share`/`member` edges). "Give the
  ops team these pages" is: define a **role** (cap bundle, shipped) + share a **nav** to the team — the
  role grants, the nav shapes the menu. Rejected: embedding caps in nav items (see Intent).
- **No per-entry cap authoring** — an entry carries no `caps[]` and cannot widen reach.
- **No cross-workspace nav** — a nav is a workspace asset; the wall holds (§6.6, rule 6).
- **No deep nesting** — one `group` level only. Trees are a later ask if needed.
- **No nav for anonymous/logged-out** — resolution requires an authenticated principal.
- **No extension-authored navs in v1** — extensions contribute *pages* (via `ext.list`, unchanged);
  they don't author menus. Deferred, named.

## Intent / approach

**A nav is just another workspace asset, cloned from the `dashboard` pattern.** By reusing the shipped
asset shape (slug id, `owner`, `visibility`, versioned record) and the shipped `share`-to-team edges, we
get workspace isolation, team assignment, and the three-gate wall **for free** — no new authz
substrate. Teams are already first-class in `lb-authz` (`teams.create/add_member`, grant subjects
`user:|team:|role:`); a nav shared to `team:ops` is visible to its members by the exact mechanism a
dashboard shared to a team already is.

**Two independent gates, never a third.** (1) *Which nav you see* = the existing visibility/share model
— resolution walks personal-pick → team-shared → workspace-default → built-in fallback. (2) *What
renders inside it* = each item filtered by the caps the caller **already holds** (a `surface` item
against `allowedSurfaces(caps)`; a `dashboard` item against that dashboard's gate-3 visibility). The nav
is a **lens**, and the server re-checks every verb on click regardless.

**Why not embed caps in nav items?** (rejected alternative) It would duplicate `lb-authz`, create a
second source of truth for "can Ada reach the rules page," and violate capability-first (rule 5): a menu
record must never be the thing that makes a page reachable. Keeping the nav a pure lens means a stale or
over-eager nav can only *show a link that then 403s server-side* — it can never *grant*.

**Why not put per-user nav in `lb-prefs`?** (rejected alternative) `lb-prefs` has a **deliberately
closed axis set** (formatting/localization only) and **no per-team tier** and **no share attachment**. A
nav blob doesn't fit its schema and shouldn't force it open. The active-pick is the only genuinely
prefs-shaped piece, and even that lives in the nav module's own `nav_pref` record rather than bending
the prefs axis set.

## How it fits the core

- **Tenancy / isolation (rule 6):** every `nav` and `nav_pref` key is `[ws, …]`, namespaced by
  workspace like `dashboard`. `nav.resolve` and every verb use the authenticated `ws`; ws-B can never
  read or resolve ws-A's navs. Tested (mandatory isolation test).
- **Capabilities (rule 5):** new caps, mirroring the dashboard set —
  - `mcp:nav.list:call`, `mcp:nav.get:call`, `mcp:nav.resolve:call` — **member-level** reads (every
    member resolves their own menu).
  - `mcp:nav.save:call`, `mcp:nav.delete:call`, `mcp:nav.share:call` — **admin-ish writes**; the
    `workspace-admin` built-in role gets them by default (revocable like any grant, no bypass — rule 10).
  - Store caps `store:nav:read|write` and `store:nav_pref:read|write` (the latter member-owned: a member
    always curates their own pick).
  - **Deny path:** a caller without `mcp:nav.save:call` calling `nav.save` → denied at gate 2, nothing
    persists. A caller resolving a nav that lists the `rules` surface but lacking `rules.*` → the item is
    **stripped by `nav.resolve`** (lens) **and** the route's `CoreGate`/server re-check denies direct
    access (defense in depth). Both are tested.
- **Placement:** either (symmetric, rule 1). A nav is store state + cap checks + tag lookups — no
  cloud-only path, no `if cloud`. Resolves the same on an edge node as in the cloud.
- **MCP surface (API shape §6.1):**
  - **CRUD** — `nav.save` (create+update, LWW by slug), `nav.delete` (soft-delete tombstone like
    dashboard), `nav.share` (add/remove a team share edge). Each its own file/tool/cap (FILE-LAYOUT).
  - **Get / list** — `nav.get{id}` (full record) and `nav.list` (cheap `NavSummary`: id/title/visibility/
    updated_ts, no `items[]` bodies), workspace-scoped, `store:nav:read`.
  - **Resolve** — `nav.resolve{}` — the one composite read: pick + tag-expand + cap-strip. Read-only,
    member-level.
  - **Live feed** — **N/A for the record itself** (a menu changes rarely; the UI reloads `nav.resolve` on
    focus/visit, like the dashboard cache does). But a `tag-group` entry's membership *does* move as
    dashboards get tagged — that motion already flows through the shipped tags/series planes; the nav
    just re-resolves on visit. No new SSE route.
  - **Batch** — N/A. Navs are authored one at a time; no bulk import in v1.
- **Data (SurrealDB, rule 2):** one new table `nav` (SCHEMAFULL: `id`, `title`, `owner`, `visibility`,
  `items[]` as a typed nested array, `schema_version`, `updated_ts`, `deleted`) and one `nav_pref`
  (composite id `[ws, user]`). Share edges reuse the existing `share`/`member` edge tables. No new
  persistence layer. `items[]` is typed/queryable, **not** a JSON blob.
- **Bus (Zenoh, rule 3):** N/A directly — a nav is **state**, not motion. Tag-group dynamism rides the
  existing tags plane; nothing is published as a nav event in v1.
- **Sync / authority:** node-local store, same sync posture as `dashboard`. Offline: `nav.resolve` reads
  the local store; a nav authored offline is an idempotent LWW upsert by slug (composite id → clean
  merge), like agent-memory/dashboard.
- **Secrets:** N/A — no secret material.

## Example flow

1. An admin (holds `workspace-admin` role → `mcp:nav.save:call`) opens the **nav builder**. They create
   `nav:ops` titled "Operations".
2. They add items: a `surface` entry for `channels`; a `dashboard` entry for `dashboard:cooler-health`;
   a `tag-group` `{ label: "Sites", facets: [{ key: "site" }] }`; a `group` "Admin" containing the
   `rules` and `flows` surfaces.
3. They set `visibility: team` and call `nav.share{ id: "nav:ops", team: "team:ops" }` → a
   `nav -[share]-> team:ops` edge is written.
4. **Ada** (member of `team:ops`, holds `rules.*` and `dashboard:cooler-health` read, but **not**
   `flows.*`) logs in. NavRail calls `nav.resolve`:
   - Pick: no personal pick → first team-shared nav for her teams → `nav:ops`.
   - `tag-group "Sites"` expands via `tags.find({facets:[{key:"site"}]})` → the three site dashboards
     she can read.
   - The `flows` item inside "Admin" is **stripped** (she lacks `flows.*`); `rules` stays.
5. Ada sees: Channels · Cooler Health · Sites ▸ (Plant-1, Plant-2, Plant-3) · Admin ▸ (Rules). She clicks
   Rules → the route loads; the gateway re-checks `rules.*` and allows it.
6. **Ben** (also `team:ops`, but lacks `dashboard:cooler-health`) resolves the *same* `nav:ops` and the
   Cooler Health entry is stripped for him — same menu record, different lens.
7. A new dashboard gets tagged `site:plant-4`. Next time Ada visits, the "Sites" group shows Plant-4 —
   **no nav edit** occurred.

## Testing plan

Per `scope/testing/testing-scope.md`, against the **real** store/caps/gateway seeded with **real**
records (no mocks, rule 9):

- **Capability deny (mandatory)** — `nav.save`/`nav.delete`/`nav.share` denied without their cap
  (nothing persists); `nav.resolve` denied without `mcp:nav.resolve:call`. Per-verb MCP deny over the
  bridge.
- **Workspace isolation (mandatory)** — ws-B cannot `get`/`list`/`resolve` a nav authored in ws-A;
  `nav_pref` isolation (ws-B user can't read ws-A's pick).
- **The "nav never widens" test (headline)** — a nav lists the `rules` surface and a dashboard the caller
  lacks; `nav.resolve` **strips both** AND a direct route/verb call to those pages is still **denied**
  server-side. Proves the lens grants nothing.
- **Resolution precedence** — personal pick > team-shared > workspace-default > built-in `SURFACES`
  fallback; empty state (no nav at all) yields the built-in fallback, never blank.
- **Tag-group dynamism** — tag a new dashboard with a facet in a nav's tag-group → it appears on
  re-resolve; untag → it disappears; tag-group only surfaces dashboards the caller can read.
- **Member-owned pref** — a member sets their own `nav_pref` (no admin cap); cannot set another user's.
- **Idempotent upsert** — `nav.save` twice by slug is LWW, no duplicate; offline-authored nav merges
  cleanly.
- **UI (real spawned gateway, `pnpm test:gateway`)** — NavRail renders `nav.resolve` output; falls back
  to `SURFACES` when none; builder round-trips a nav (pick surfaces/dashboards/ext pages + a tag-group +
  a team share) save→reload→resolve; the cap-strip is visible in the rendered rail.

## Risks & hard problems

- **The lens-vs-grant boundary is the whole design** — the one bug that matters is a nav item that
  *becomes* a way to reach a page. The "nav never widens" test and the server-side re-check on every
  route are the guardrails; keep `nav.resolve` a pure filter over caps the caller already holds, and
  never let the builder write a cap.
- **Rule 10 (core knows no extension)** — `ext` entries reference an **opaque** id resolved via the
  generic `ext.list` discovery; NavRail must not branch on any ext id (no icons/routes keyed to a named
  ext). The item stores `ext` as data; rendering goes through the existing `ExtHost`/`useExtensionPages`
  seam.
- **Resolution cost** — `nav.resolve` fans out to `tags.find` per tag-group and visibility checks per
  item. Bound it: cap the item count per nav, resolve tag-groups with the existing (cached) tag path,
  and lean on the shipped dashboard read-cache pattern (`features/dashboard/cache/`) so a visit resolves
  once. State the item cap.
- **Fallback correctness** — the built-in `SURFACES` fallback must stay in lockstep with the shipped
  surface set so a workspace with no nav is identical to today. Drive the fallback from the same
  `SURFACES` source, not a copy.
- **Stale pick** — a `nav_pref` pointing at a deleted/unshared nav must fall through to the next
  resolution tier, not error.

## Open questions

- **Where does the builder live** — a new admin surface, or a tab under the existing access-console
  (`scope/auth-caps/access-console-scope.md`)? Lean: a tab in access-console (it's an authz-adjacent
  authoring tool), with its own cap-gated route.
- **Item cap per nav** — pick a bound (proposal: 100 items incl. expanded tag-group results capped
  separately, e.g. 50 per group) and enforce it in `nav.save` / `nav.resolve`.
- **Workspace-default nav** — is "the workspace default" a `visibility:workspace` nav (first/most-recent
  wins), or an explicit `nav.set_default{id}` admin verb (like `prefs.set_default`)? Lean: explicit
  `nav.set_default` for determinism; resolve reads a single `workspace_nav_default:[ws]` pointer.
- **`nav_pref` vs a `lb-prefs` axis** — confirm the dedicated `nav_pref` record over adding a closed
  `active_nav` axis to prefs. Lean: dedicated record (keeps the prefs axis set closed).
- **Ext-page entries and uninstalled extensions** — if a nav lists an `ext` page whose extension is no
  longer installed (`ext.list` doesn't return it), strip it silently (like a cap-stripped item) — confirm.
- **Ordering & grouping persistence shape** — `items[]` as a flat ordered array with `group` children
  vs. a separate order index. Lean: flat ordered array (one source of truth, matches `dashboard.cells`).

## Related

- README `§3` (the non-negotiable rules — 5 capability-first, 6 workspace wall, 10 core-knows-no-ext),
  `§6.5` (dashboards / assets), the MCP contract (`§7`).
- `scope/frontend/dashboard-scope.md` — the asset + `share`-to-team model this clones (`dashboard/share.rs`).
- `scope/frontend/dashboard/library-panels-scope.md` — standalone panel pages (`/panel/{id}`) are a
  natural later `panel` entry kind alongside `dashboard`.
- `scope/auth-caps/authz-grants-scope.md` — teams/roles/grants (`user:|team:|role:` subjects,
  `role`/`team` primitives) this attaches to; `scope/auth-caps/access-console-scope.md` (likely builder home);
  `scope/auth-caps/admin-crud-scope.md` (team management).
- `scope/tags/tags-scope.md` — `tags.add/find` for dynamic tag-group entries and nav discovery.
- `scope/prefs/user-prefs-scope.md` — why the active pick lives in `nav_pref`, not a prefs axis.
- `scope/frontend/nav-rail-scope.md` — the **panel section rail** (distinct: not user nav — do not confuse).
- `scope/frontend/routing-scope.md` — `SURFACES`, `allowedSurfaces`, `CoreGate`, `ext:<id>` routing this renders through.
- Skill: `skills/nav/SKILL.md` (see §6 below — required, drivable surface).
