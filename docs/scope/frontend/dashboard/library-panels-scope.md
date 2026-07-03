# Dashboard scope — library panels (panels as their own asset, reusable + standalone)

Status: scope (the ask). Promotes to `public/frontend/dashboard.md` → "Library panels" once shipped.
Builds directly on the shipped v3 panel model (`viz/panel-model-scope.md`), the `dashboard.*` verbs,
and the S4 asset-sharing model.

Today a chart/widget exists **only as a `Cell` embedded in one dashboard's `cells[]`**
(`rust/crates/host/src/dashboard/model.rs` — there is no panel table). To use the same chart on two
dashboards you duplicate its JSON; when the source changes you edit N copies; and a panel cannot render
anywhere *except* inside a dashboard grid. We want **panels as their own asset**: a `panel:{id}` record
that (a) many dashboards **reference** — edit once, every referencing dashboard updates — and (b)
renders **standalone**, on its own page, with no dashboard at all (a directly linkable chart — e.g. a
nav entry, a shared link, a future channel embed). Like everything else here it is a **lens over
existing data access, never a grant path**: sharing a panel shares its *definition*; the data it reads
is still re-checked per call against the **viewer's** caps.

The load-bearing observation: a v3 `Cell` is already cleanly separable. **Layout** (`i,x,y,w,h`) is
per-dashboard placement; **everything else** (`view`, `title`, `description`, `sources[]`,
`transformations`, `fieldConfig`, `options`, `action`, `binding`/`source` for v1/v2) is the panel
spec. This scope lifts the spec into its own record and leaves placement where it belongs.

> **No backwards migration required.** There is no production dashboard data to preserve — existing
> dashboards may be **deleted and re-created** if that makes the cleanest design (confirmed by the
> owner). We are NOT obligated to keep old dashboard records readable, to write a migrator, or to bend
> the record shape to stay round-trip-compatible with what's on disk today. The `panel_ref` field is
> still **additive** on `Cell` (so an inline cell and a ref cell coexist by design — that's the
> feature, not a compat concession), but if a decision here is cleaner with a schema bump + a wipe of
> the current seed/dev dashboards, take it. This removes migration from the risk list entirely.

---

## Goals

- **A `panel` asset** — a workspace-namespaced `panel:{id}` record (stable slug, unique per workspace),
  modeled like `dashboard`: `title`, `owner`, `visibility (private | team | workspace)`, shared to a
  team via the existing S4 `share` edge, soft-delete tombstone, `schema_version` pinned to the **same
  v3 panel model** — the spec is exactly the non-layout portion of today's `Cell` (typed nested object,
  queryable, no app-side JSON parsing).
- **Full panel CRUD as host MCP verbs** — `panel.get` / `panel.list` / `panel.save` (create+update,
  LWW by slug) / `panel.delete` / `panel.share`, each capability-gated and workspace-first, wired end
  to end (store → cap → MCP → gateway route → `http.ts` → UI), plus **`panel.usage{id}`** (which
  dashboards reference it — the delete-safety and "where is this used" read).
- **Reference cells** — an additive `panel_ref: "panel:{id}"` field on `Cell`
  (`#[serde(default)]`, empty = inline cell, unchanged). A ref cell carries **layout + the ref +
  bounded per-placement overrides** (title override, variable bindings) and **no spec**;
  `dashboard.get` hydrates the spec from the panel record at read time, **host-side** (see
  Decisions). Inline
  cells remain first-class forever (a one-off chart should NOT be forced to become an asset) — but
  this is a **design choice, not a migration constraint**: there's no obligation to keep on-disk
  dashboards readable across the change (see "No backwards migration" above), only to keep inline and
  ref cells coexisting going forward.
- **Link / unlink in the editor** — the shipped panel editor gains: "Save as library panel" (extract
  this cell's spec → `panel.save`, cell becomes a ref), "Add library panel" in the builder's source/
  widget picker (`panel.list`), and "Unlink" (copy the spec back inline — fork, stop tracking).
  Editing a library panel from within any dashboard edits **the shared record** (with a visible
  "library panel — used on N dashboards" banner from `panel.usage`).
- **Standalone panel page** — a `/t/$ws/panel/{id}` route rendering ONE panel full-bleed through the
  **same** shipped render path (`PanelView`/`usePanelData`/the viz bridge — no parallel renderer),
  cap-gated like every core route. This is the "chart not on a dashboard" ask, and the page a nav
  entry or a shared link points at.
- **Time range / variables on the standalone page** — the page carries its own range picker +
  `?var-<name>=` URL selections (the shipped variable model), since there is no host dashboard to
  supply them; sensible defaults from the panel spec.

## Non-goals

- **No migration, in either direction.** We don't force inline cells to become panels (refs are opt-in
  per cell), AND we don't owe old on-disk dashboards compatibility — there's no production data, so a
  schema bump + re-seed is fair game (see the callout in Intent). Migration is simply not a
  requirement here.
- **No per-dashboard spec overrides beyond the bounded set** (title, variable bindings). If a
  placement needs different queries/options, **unlink** — a half-shared spec with deep per-placement
  patches is the complexity that sinks this feature. (Grafana's library panels made the same call.)
- **No panel versioning/history in v1** — LWW like dashboards. Named deferral (pairs with the
  import/export scope's fidelity work).
- **No cross-workspace panels** — the wall holds (rule 6).
- **No new render path** — the standalone page and hydrated ref cells reuse the shipped
  `PanelView`/`usePanelData`/viz bridge verbatim.
- **No channel/GenUI embedding in v1** — the standalone route is the one new render host; channel
  message embeds and GenUI references are named deferrals.

## Intent / approach

**Clone the `dashboard` asset pattern one level down.** `panel` gets the identical treatment
dashboards got — slug id, owner, S4 visibility + `share` edges, cap-gated verbs, tombstone delete — so
isolation, sharing, and the three-gate wall come for free from the shipped substrate. The dashboard's
`cells[]` keeps **placement** (grid geometry is meaningless outside its grid) and gains a pointer.

**Reference + hydrate, not copy-on-use.** A ref cell's spec is resolved from the panel record at read
time, so an edit to the panel propagates to every dashboard on next load — that IS the feature ("edit
once, reuse everywhere"). *Rejected alternative — copy-on-insert with a "sync" button:* it silently
drifts, defeats the single-source-of-truth ask, and every sync is a manual N-dashboard chore; if a user
wants drift, that's exactly what **Unlink** is for, as an explicit act.

**Data access stays with the viewer, definition access with the asset.** `panel.get` passes the
three-gate read on the panel record (ws → cap → visibility). But the panel's `sources[]` execute under
the **viewer's** caps through the existing per-call re-check (`viz.query`/`cellTools` leash) — exactly
as an inline cell does today. A panel shared to the workspace whose query needs `series:hvac.*:read`
renders as "denied/no data" for a viewer without that cap. Sharing a panel **never widens data
access** — same thesis as the nav scope (`scope/nav/nav-builder-scope.md`).

**Hydration lives in one seam.** One resolver — **host-side, on `dashboard.get`** (Decisions) —
expands `panel_ref` → spec, so the grid, the editor, the read cache, and the
standalone page all see plain v3 panels and need no ref-awareness beyond the editor's link/unlink
affordances.

## How it fits the core

- **Tenancy / isolation (rule 6):** `panel:{id}` is workspace-namespaced like `dashboard:{id}`; every
  verb and the hydration resolver use the authenticated `ws`. A ref can only resolve within its own
  workspace. Tested (mandatory).
- **Capabilities (rule 5):** mirrors the dashboard set — `mcp:panel.list|get|save|delete|share|usage:call`
  + `store:panel:read|write`. Read caps default to wherever `dashboard.list` sits today (a panel is not
  more sensitive than the dashboard embedding it); writes to editors. **Deny path:** `panel.save`
  without the cap → gate-2 deny, nothing persists; a hydrated ref whose panel the viewer cannot read →
  the cell renders an honest "panel not accessible" placeholder (never leaks the spec); the panel's
  *data* tools re-checked per call as today.
- **Symmetric nodes (rule 1):** store + caps + one resolver; no cloud branch.
- **One datastore (rule 2):** one new SCHEMAFULL `panel` table; `share` edges reused; no new
  persistence.
- **State vs motion (rule 3):** a panel is state. Live data keeps flowing through the shipped series
  SSE/bus paths untouched. **No "panel changed" bus event in v1** — dashboards re-hydrate on
  load/visit (the dashboard read-cache already scopes to the visit); a live "panel updated while you
  watch" push is a named deferral.
- **Stateless extensions (rule 4):** N/A — no extension holds panel state; `ext:<id>/<widget>` view
  types work in a library panel unchanged (the ref is one more level of indirection *above* the widget
  contract, which does not change).
- **MCP surface (API shape §6.1):** CRUD (`save`/`delete`/`share`) + get/list (`get` full spec, `list`
  cheap summary: id/title/view/visibility/updated_ts + usage count) + the one extra read `usage`.
  **Live feed:** N/A (see state vs motion). **Batch:** N/A in v1 — panels are authored one at a time;
  bulk import rides the existing viz `import-export-scope.md` when that lands (a Grafana library-panel
  import maps here — flag it there).
- **Durability:** N/A — no must-deliver effects; all reads/writes are direct store ops.
- **SDK/WIT impact:** none — the widget/federation contract is untouched; `panel_ref` is additive on
  `Cell` and invisible below the hydration seam.
- **One responsibility per file:** `crates/host/src/panel/` mirrors `dashboard/` (`model.rs`, one verb
  per file, `share.rs`, `usage.rs`); UI: `features/panel/` for the standalone page, editor affordances
  in the existing editor files.
- **Skill doc (§6):** required — `skills/panels/SKILL.md` (create a library panel from a cell, reuse it
  on a second dashboard, drive `panel.*` headlessly, open the standalone page), written by the
  implementing session from a live run.

## Example flow

1. Ada builds a "Cooler temp (24h)" chart on `dashboard:ops`. In the panel editor she hits **Save as
   library panel** → `panel.save` writes `panel:cooler-temp-24h` (spec extracted); her cell becomes
   `{ i,x,y,w,h, panel_ref: "panel:cooler-temp-24h" }`.
2. She sets the panel `visibility: workspace`. On `dashboard:exec-summary` she picks **Add library
   panel** → `panel.list` → inserts a ref cell; only geometry is authored there.
3. A month later the sensor series is renamed. Ada edits the panel **once** (from either dashboard, or
   the standalone page — the editor banner shows "library panel — used on 2 dashboards" via
   `panel.usage`). Both dashboards show the fix on next load.
4. Ben (no `series:hvac.*:read`) opens `dashboard:exec-summary`: the ref hydrates (he can read the
   panel record) but `viz.query` denies the data — the cell shows the honest deny, exactly as an
   inline cell would.
5. Ada shares the direct link `/t/ops/panel/cooler-temp-24h` in a channel; it renders the panel
   full-bleed with its own range picker — no dashboard involved. (Later, a nav entry can point at the
   same page.)
6. The exec dashboard needs a variant with a different threshold: Ada duplicates the ref and hits
   **Unlink** on the copy — the spec is copied inline, drift is now explicit and hers.
7. She tries `panel.delete` on a panel used by 2 dashboards → refused with the usage list; `force:true`
   tombstones it and referencing cells render the "panel deleted" placeholder until relinked/removed.

## Testing plan

Per `scope/testing/testing-scope.md`, real store/caps/gateway, real seeded records (rule 9):

- **Capability deny (mandatory):** each `panel.*` verb denied without its cap (nothing persists);
  per-verb MCP-bridge deny; ref-to-unreadable-panel renders the placeholder, never the spec.
- **Workspace isolation (mandatory):** ws-B cannot `get`/`list` ws-A panels; a ws-B dashboard cell
  with a ws-A `panel_ref` does **not** hydrate (the cross-ws ref test is the headline isolation case).
- **"Sharing never widens data access" (headline):** workspace-visible panel + viewer lacking the
  source cap → definition readable, data denied at `viz.query` — asserted at the real gateway.
- **Inline/ref coexistence (not on-disk compat):** an inline cell and a ref cell coexist on the same
  dashboard and each round-trips through `save`/`get` unchanged; an inline cell never silently grows a
  `panel_ref`. (We do NOT test old-on-disk-dashboard compatibility — there's no data to preserve; a
  schema bump may re-seed dev/demo dashboards.)
- **Propagation:** edit panel → both referencing dashboards reflect it on reload; unlink → edits stop
  propagating to the forked cell.
- **Delete safety:** delete-in-use refused with usage list; forced tombstone → placeholder cell; new
  save un-hides (dashboard tombstone semantics).
- **Bounds:** `panel.save` enforces the same record-growth bounds `dashboard.save` applies to cells.
- **UI (`pnpm test:gateway`):** save-as-library → reuse on a second dashboard → edit-once-propagates,
  authored through the real editor; the standalone route renders with range + `?var-` selections;
  read-cache de-dup (N ref cells to one panel on one dashboard visit → one hydration).

## Risks & hard problems

- **Edit-propagation surprise** — a user "just tweaking a chart" silently changes another team's
  dashboard. The banner (`panel.usage`) + explicit Unlink are the mitigation; the editor must never
  hide that a cell is a ref.
- **The override boundary** — pressure will mount to allow "just this one option" per placement.
  Hold the bounded set (title, variable bindings); everything else is Unlink. A patch-merge layer is
  the tar pit.
- **Hydration and the read cache** — the dashboard visit cache keys on the resolved spec
  (`dashboard-query-cache-scope.md`); hydration must happen **before** cache keying so two ref cells
  to one panel de-dup, and a panel edit must invalidate cleanly (cache scoped to the visit already
  bounds staleness).
- **Dangling refs** — deleted/unshared/missing panels must degrade to an honest placeholder in every
  host (grid, editor, standalone), never a crash or a silent empty chart.
- **Ref cycles are impossible by construction** (panels cannot reference panels) — keep it that way;
  a "panel of panels" is a dashboard.

## Decisions (open questions resolved — 2026-07-03, owner-approved: "best long term")

- **Hydration seam: host-side.** `dashboard.get` returns **hydrated** cells (the resolved v3 spec)
  with the `panel_ref` kept on each ref cell as the marker. Why: headless MCP callers, export, and any
  future render host (channels, GenUI, nav) get resolved dashboards for free; the placeholder/
  isolation/deny logic lives behind the capability wall exactly once, instead of being re-implemented
  per client. The UI editor uses the marker for its link/unlink affordances; `dashboard.save` accepts
  a ref cell **without** spec (the ref is authoritative — a stale hydrated copy sent back by a client
  is ignored, preventing accidental de-linking). *Rejected:* UI-adapter hydration — it makes every
  non-shell caller (agent, CLI, export) ref-aware and duplicates the deny/placeholder logic outside
  the wall.
- **`dashboard.save` validates refs: yes.** A save whose `panel_ref` doesn't resolve in-workspace is
  rejected loudly at author time (`BadInput`, naming the ref). Later dangling (panel force-deleted
  after the save) degrades to the placeholder at hydration — validate at write, tolerate at read.
- **Standalone-page cap: distinct.** The `/panel/{id}` route gates on `store:panel:read` (surfaced
  through `allowedSurfaces` like every core route); panel record reads pass the normal three gates.
  No piggybacking on `dashboard.list` caps — panels are their own asset and get their own read cap.
- **`panel.list` summary: cheap, no usage count.** `PanelSummary` = id/title/view/visibility/
  updated_ts (the `DashboardSummary` precedent); usage is computed on demand by `panel.usage{id}`
  (the editor banner and delete-safety call it when needed). No per-row usage query on every list.
- **Slug: title is free, slug is forever.** The extract flow derives the slug from the title +
  a disambiguator on collision, shown once and editable **at creation only**; after that, rename
  changes `title` only (dashboards ref by id — the dashboard precedent). No slug-rename/aliasing.

## Open questions

None — resolved above. Anything new the build surfaces goes here per HOW-TO-CODE.

## Related

- README `§3` (rules 2, 5, 6), `§6.5`.
- `viz/panel-model-scope.md` — the v3 spec this record stores; `viz/import-export-scope.md` (Grafana
  library-panel import maps here); `viz/panel-editor-scope.md` + `editor-parity-scope.md` (the editor
  gaining link/unlink); `dashboard-query-cache-scope.md` (hydration × cache).
- `../dashboard-scope.md` — the asset pattern cloned (verbs, share edges, tombstones).
- `scope/nav/nav-builder-scope.md` — the standalone panel page is a natural nav-entry target (a
  possible later `panel` entry kind alongside `dashboard`).
- `scope/auth-caps/authz-grants-scope.md` — teams/shares the panel asset rides.
- Skill: `skills/panels/SKILL.md` (required — drivable `panel.*` surface; written on ship).
