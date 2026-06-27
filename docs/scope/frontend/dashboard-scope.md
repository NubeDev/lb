# Frontend scope — the dashboard surface (grid of widgets over real series)

Status: scope (the ask). Promotes to `public/frontend/dashboard.md` once shipped. Target stage:
**S9+ collaboration UI** (builds directly on the shipped S8 data plane — `series.read`/`series.latest`/
`series.find` — and the shipped S9 real-session shell). This is the worked example
`vision/0003-iot-dashboard.md` made buildable, with the fleet deferred.

We want a **first-party dashboard surface in the shell**: a logged-in user opens a workspace, sees
dashboards of its real series, **drags and resizes widget tiles** on a grid, and watches charts update
**live**. Everything runs against a **real node** — real store, real series, real capability checks,
real Zenoh motion — seeded with **real records** through the real ingest write path (no fleet hardware
yet, no mocks; CLAUDE §9). The dashboard *core* (the grid, the layout records, the widget host) is
trusted shell code; **widgets are the unit that later crosses the extension trust boundary**
(`ui-federation-scope.md`) — so this scope ships the widget *data-binding contract* first-party, proving
it before federation exists.

This scope is **three sequenced phases**. **Phase 1 (this doc's build-ready ask)** is the core dashboard
with built-in widgets over seeded series. **Phase 2** moves the widget contract behind the federation
bridge (a narrowing of `ui-federation-scope.md` to widget-in-a-cell). **Phase 3** replaces seeded data
with a real edge fleet (existing `node-connection`/`fleet-presence`/`authz-grants` scopes). Phases 2–3
are roadmapped here with their dependencies and de-risked decisions; only Phase 1 is specified to the
level of "code it with no open questions."

---

## Goals

### Phase 1 — Browser-on-hub dashboard, seeded (the build-ready ask)

- **A seeded, realistic series fixture** written through the **real ingest path** — a `seed_iot_demo`
  entrypoint that emits real `Sample`s into real `series` tables (walk-in-cooler temps, fryer state),
  tagged `store:…`, `kind:temperature`, `equipment:walk-in-cooler`, in workspace `kfc`. This is "real
  data without a real sensor" — it makes every later test and demo honest (the no-mocks/seed rule).
- **A core dashboard surface** (`ui/src/features/dashboard/`) — a `react-grid-layout` grid host with a
  cap-gated nav slot, a `DashboardView` of draggable/resizable cells, the layout **persisted as a
  SurrealDB record** per workspace (`dashboard:{id}` — state in the store, never `localStorage`).
- **Full dashboard CRUD as host MCP verbs** — `dashboard.get` / `dashboard.list` / `dashboard.save`
  (create+update) / `dashboard.delete`, each capability-gated and workspace-first, wired end to end
  (store → cap → MCP → gateway route → `http.ts` → UI). The *complete* surface, not a read-only subset
  (HOW-TO-CODE §3 step 4a).
- **A small set of first-party widget types** bound to real series — **time-series chart**, **stat /
  single-value**, and **gauge** — that read `series.read` / `series.latest` (history, state) and
  subscribe to the **live Zenoh series stream** for updates (motion). These ship *in the shell first*,
  proving the data-binding contract before any federation.
- **A live series SSE route** — `GET /series/{series}/stream` on the gateway (the series analog of the
  channel stream that already exists), so a browser widget receives live sample motion without polling
  `series.latest` on a timer (state vs motion, rule 3).
- **A widget data-binding contract** — a cell holds `{ widget_type, binding, options }`, where a
  `binding` is **either an explicit `series` name or a tag facet query** (resolved via the shipped
  `series.find`). This is the contract Phase 2 moves behind the bridge unchanged.

### Phase 2 — Widgets as installed extensions (the federation layer) — roadmap

- Move the Phase-1 widget contract **behind the host-mediated `postMessage` bridge** of
  `ui-federation-scope.md`, **narrowed to widget-in-a-cell**: a widget gets one grid cell, a **read-only**
  data binding (`series.read`/`series.latest`/`series.watch` within its bound scope), and **nothing
  else**. No session token, no arbitrary tools, no nav page. This is the **smaller 60%** of the
  federation scope — the load-bearing trust surface shrinks to "render data it was bound to, call
  nothing else."
- A `[widget]` manifest declaration (entry, label, the read-only series scope it may bind) + the cell
  host rendering a widget **first-party** (Phase 1) or **federated** (trusted key → in-process; untrusted
  → sandboxed iframe), through the bridge. One Phase-1 widget ported to a federated extension
  (`chart-widget`) as the reference.

### Phase 3 — The real edge fleet — roadmap

- Replace the seed with real producers: **node connection** (`node-connection-scope.md`), **fleet
  presence** (`fleet-presence-scope.md`), a **`sensor-source` native extension** (the first real
  producer), an **`alerts` extension** (threshold breach → inbox item → outbox notification), and
  **grant-by-tag** (`authz-grants-scope.md` — "operators see all appliances tagged `region:emea`").

---

## Non-goals

- **No new datastore or persistence layer.** Layouts are SurrealDB records; series already live in
  SurrealDB. No `localStorage` for durable state, no TSDB, no separate dashboard DB.
- **No widget federation in Phase 1.** Phase 1 widgets are **first-party, in-shell** React components.
  The federation bridge is Phase 2 — but the Phase-1 widget *binding contract* is designed to move
  behind it unchanged. (Building federation first would over-build the trust surface before the contract
  is proven.)
- **No fleet hardware / protocol bridges in Phase 1.** Data is seeded through the real ingest path. The
  real fleet is Phase 3; `sensor-source`/MQTT/Modbus stay out-of-core extensions (ingest scope rule).
- **No `if cloud {…}`.** The same dashboard app the `workstation` runs in Tauri is the app the `hub`
  serves to a `browser` (`vision/0003` §4) — delivery differs (Tauri `invoke` vs SSE/HTTP), the app does
  not. The gateway route and the Tauri command call the **same** host verb.
- **No analytics / compute plane.** Widgets read series and rollups; aggregation is SurrealDB view /
  `series.read` shaping, not a new compute engine (ingest scope non-goal).
- **No *new* sharing mechanism** — a dashboard is an **asset**, so it reuses the **shipped S4 asset
  sharing model** (`share_doc`/`link_doc`/membership three-gate, `scope/files/`) rather than inventing
  a parallel ACL. Per-dashboard visibility (private → shared-to-team → workspace) is **in Phase 1**, not
  deferred — see "Access & authorization" below. What is deferred (named, not silent): row-level
  per-*widget* visibility within a shared dashboard (a viewer simply sees only the cells whose series
  they're granted — the cell renders a denied state otherwise).
- **No `*.fake.ts`.** Tests run against a real in-process gateway seeded via the real write path (the
  retirement rule — STATUS "Next up" item 00).

---

## Intent / approach

**Dashboard core in the trusted shell; widgets are the cell-sized unit that later federates.** The shell
gains a `features/dashboard/` surface: a `react-grid-layout` host renders cells from a persisted layout
record; each cell names a `widget_type` and a `binding`. A widget is a tiny, constrained thing — one
cell, a read-only series binding, render. That smallness is the whole design bet: it is the easy,
safe-to-federate-later unit (this scope's core insight). So Phase 1 proves the binding contract with
**built-in** widgets; Phase 2 swaps the *renderer* (first-party component → federated remote / iframe)
without changing the *contract* (`{widget_type, binding, options}` + read-only series reads).

```
  seed_iot_demo ──(real ingest write path)──► series tables (real Samples, tagged)
        │
        ▼
  dashboard:{id} record (layout: cells[]) ──dashboard.get──► DashboardView (react-grid-layout)
        │                                                          │ per cell
        │  drag/resize ──dashboard.save──► record                 ▼
        │                                              WidgetHost({widget_type, binding})
        │                                                  │            │
        │                              history (state)     │            │  live (motion)
        │                              series.read /        ▼            ▼
        │                              series.latest ◄── HTTP      GET /series/{s}/stream (SSE)
        │                              series.find  ◄── (tag binding)     ▲
        └───────────────────────────────────────────────────► Zenoh ws/{id}/series/{series}
```

- **Layout is state (SurrealDB), live values are motion (Zenoh).** The grid layout — which cells, where,
  what binding — is a durable record read with `dashboard.get`. The chart's moving line is the Zenoh
  series stream over SSE. History (the chart's initial range, the stat's last value) is a `series.read`/
  `series.latest` store query. Three altitudes, kept distinct (rule 3) — never a `setInterval` poll on
  `list`/`latest` for "live".
- **The widget binding is the forever-contract.** `binding = { series: "node.cpu_temp" }` **or**
  `binding = { find: { tags: ["kind:temperature", "store:downtown-0421"] } }` (resolved via the shipped
  `series.find`). A widget never names a tool other than the three read verbs; this is exactly the
  read-only scope Phase 2 enforces at the bridge. Designing it now means Phase 2 is a renderer swap.
- **One host service, mirrored over the gateway.** `dashboard.*` follows the exact shipped pattern
  (`ingest`/`channel_registry`/`workflow`): a `crates/host/src/dashboard/` service (authorize → raw
  verb), a `routes/dashboard.rs` gateway mirror that re-checks the cap server-side and takes ws+principal
  from the **token, not the body** (§7), and an `http.ts`/`dashboard.api.ts` client mirroring the verbs
  1:1.

**Rejected alternatives:**

- *Layout in `localStorage`.* Rejected — durable state must live in SurrealDB or on the bus (rule 4,
  stateless-extensions; and a dashboard built on one device must appear on another). `localStorage` is
  not the datastore.
- *Widgets poll `series.latest` on a timer for "live".* Rejected — that uses state as motion (rule 3),
  doesn't scale, and lags. Live is the Zenoh stream over a series SSE route; the store query is only the
  initial history backfill.
- *Build the federation bridge first, then widgets.* Rejected — over-builds the load-bearing trust
  surface before the binding contract is proven. Ship first-party widgets, prove the contract, then move
  it behind the bridge (Phase 2). The vision explicitly supports "seed real data, defer the fleet."
- *Bake a "device"/"sensor" widget type into core.* Rejected — the core never knows "fryer" (ingest/
  vision rule). A widget binds a generic `series`; the IoT-ness is which series the seed (later: a
  bridge) creates and what tags it attaches.
- *A bespoke per-dashboard React tree.* Rejected — one generic grid host driven by the layout record,
  like the admin console is one generic surface over `ext.*`.

## How it fits the core

- **Tenancy / isolation (rule 6):** every `dashboard:{id}` record is in the workspace namespace; the
  series it binds are already `ws/{id}/…`. A ws-B user cannot read, list, save, or delete a ws-A
  dashboard, and a ws-B widget's bridged series reads hit only ws-B (the existing series wall). The
  two-session isolation test extends to dashboards. **Mandatory test.**
- **Capabilities (rule 5/7):** new caps `mcp:dashboard.get:call`, `mcp:dashboard.list:call`,
  `mcp:dashboard.save:call`, `mcp:dashboard.delete:call`, `mcp:dashboard.share:call`. Reads gate on
  get/list; mutations on save/delete; sharing on share. Widget data reads reuse the **existing**
  `mcp:series.read:call` / `mcp:series.latest:call` / `mcp:series.find:call`. The deny path is opaque (a
  denied caller learns nothing about which dashboards/series exist). **A deny-test per verb** (mandatory,
  HOW-TO-CODE §3 step 4a). The full who-sees-what model is in **Access & authorization** below — it is
  *part of Phase 1*, not deferred.
- **Placement / symmetric nodes (rule 1):** the dashboard app is one app, two deliveries — Tauri-local
  on the `workstation`, served over SSE/HTTP from the `hub` to a `browser`/`mobile` (`vision/0003` §4).
  The `dashboard.*` verbs and the series SSE route are role-mounted by config (the gateway already mounts
  by role), never a `if cloud` branch.
- **MCP surface — the API shape (§6.1):**
  - **CRUD:** `dashboard.save` (create+update, idempotent **UPSERT** on `dashboard:{id}` — a save with a
    fresh id creates, an existing id updates; one verb, not two) and `dashboard.delete` (tombstone-
    upsert, §6.8, idempotent — re-delete is a no-op). `save` is small/bounded (one layout record), so it
    stays **synchronous** — explicitly *not* a job (no fan-out, no long batch).
  - **Get / list:** `dashboard.get(id)` (single record, gate-3 checked) and `dashboard.list()` (exactly
    the dashboards the caller can reach — own + team-shared + workspace-visible; id+title+visibility+
    updated_ts, no cell bodies — a cheap roster). Distinct verbs.
  - **Share:** `dashboard.share(id, {visibility, team?})` — set a dashboard private/team/workspace,
    writing the **shipped S4 `share_doc` membership edge** for the team case. Idempotent. Gated by
    `mcp:dashboard.share:call`; owner/admin only.
  - **Live feed:** the **series** SSE route (`GET /series/{series}/stream`) is the live feed for widget
    values — surfaced as a `series.watch`-style stream, the bus for motion (not a poll on `list`). The
    dashboard *layout* itself is not live-fed in Phase 1 (single-editor; multi-admin live layout refresh
    is a named follow-up, same as the admin console's "live multi-admin refresh" open item).
  - **Batch:** **N/A.** A user edits one dashboard at a time; there is no bulk dashboard operation with a
    caller. Stated as N/A per §6.1 (not a silent omission).
- **Data (SurrealDB):** a `dashboard` table, record id `dashboard:{id}`, per workspace namespace:
  ```
  dashboard:{id} = {
    id:        string,            // stable slug, unique per workspace
    title:     string,
    owner:     string,            // the principal who created it (for the private→shared model)
    visibility: "private" | "team" | "workspace",   // the S4 asset sharing tiers
    cells:     Cell[],            // the grid layout + bindings (below)
    updated_ts: datetime,
    deleted:   bool,              // tombstone (soft-delete, §6.8 idempotent)
  }
  // Sharing to a team is an EDGE, not a field — reuses the shipped S4 `share_doc` membership edge
  // (`dashboard ->shared-> team`), so the existing three-gate read check applies unchanged.
  Cell = {
    i:        string,             // react-grid-layout item key (stable per cell)
    x, y, w, h: number,           // grid geometry
    widget_type: "chart" | "stat" | "gauge",   // Phase 1 built-ins; Phase 2 adds "ext:<id>"
    binding:  { series: string } | { find: { tags: string[] } },
    options:  object,             // widget-type-specific (range, unit label, thresholds) — typed per widget
  }
  ```
  No new persistence layer; the series tables are the shipped S8 ones. `cells` is a typed nested object
  (queryable, no app-side JSON parsing) — the storage discipline the ingest scope established.
- **Bus (Zenoh):** the live widget feed subscribes to the **existing** series motion subject
  `ws/{id}/series/{series}` (fire-and-forget, best-effort — a dropped live frame is fine; the durable
  copy is the committed series). No new subject. The series SSE route is the browser bridge onto it,
  exactly as the channel stream bridges channel motion.
- **Sync / authority:** a `dashboard:{id}` record is a `(table, id)` upsert — it rides the **existing**
  channel-sync path (§6.8) with no new mechanism, so a dashboard authored on the hub syncs to a
  workstation idempotently. (Wiring the dashboard table into the sync set is a one-line follow-up, same
  class as "sync the asset/job/outbox tables" in STATUS "Fit-and-finish carryover".)
- **Secrets:** none. No secret material in the dashboard surface.
- **SDK/WIT impact:** **none in Phase 1** — `dashboard.*` are host MCP tools like any other; no manifest/
  WIT change. **Phase 2's two forever-contracts** (the `[widget]` manifest block + the read-only widget
  bridge protocol) are **frozen** in `dashboard-widgets-scope.md` (versioned `v:1`), so the
  stop-and-confirm gate is already discharged — not left for the coding session.

## Access & authorization — who sees and edits a dashboard

A dashboard is an **asset**, so its access model is the **shipped S4 three-gate model** (README §6.6,
`scope/files/`), not a new ACL. Three independent gates, checked in order, every call:

- **Gate 1 — workspace (the hard wall, rule 6).** Every `dashboard:{id}` is workspace-namespaced. A ws-B
  principal cannot name, read, list, save, delete, or share a ws-A dashboard. Structural, not a flag.
- **Gate 2 — capability (rule 5/7).** The cap matrix, all re-checked server-side (the gateway re-checks;
  the UI gate is convenience):

  | Action | Cap | Who typically holds it |
  |---|---|---|
  | view a dashboard / its roster | `mcp:dashboard.get` / `.list` | any member granted dashboard-read |
  | create / edit (drag, resize, bind, rename) | `mcp:dashboard.save` | dashboard authors |
  | delete | `mcp:dashboard.delete` | authors / admins |
  | share to a team / set visibility | `mcp:dashboard.share` | the owner / admins |
  | a cell's data | `mcp:series.read` / `.latest` / `.find` | viewers granted those series |

- **Gate 3 — membership / visibility (the S4 layer below the wall).** A dashboard is `private` (owner
  only), `team` (shared to a team via the **shipped `share_doc` edge** — `dashboard ->shared-> team`,
  read by members), or `workspace` (any member with the read cap). `dashboard.list` returns exactly the
  set the caller can reach: their own + teams they're in + workspace-visible. A **non-member is denied**
  reading a team-shared dashboard — the same gate-3 deny the S4 doc-sharing test already proves, extended
  to dashboards.

**Per-widget visibility within a shared dashboard.** Sharing a dashboard does **not** widen series access:
a viewer sees a cell *render* only if they hold the cell's series read cap. A teammate without the cooler
series grant opens the shared dashboard and sees the cooler cell render an **honest denied/empty state**,
never a fake value and never a leak. So a dashboard can be shared broadly while its data stays gated
per-series — the cell is the visibility unit, the series wall is the leash. (This composes directly with
`dashboard-widgets-scope.md`'s widget access model.)

**This is Phase 1, built and tested** — the `share_doc`/membership machinery exists (S4); the work is the
`dashboard.share` verb + threading `visibility`/the share edge through `get`/`list`. The mandatory
**gate-3 non-member deny** test and the **two-session isolation** test both apply here.

## Example flow (Phase 1)

1. **Seed.** `seed_iot_demo` runs against a real node (test harness / dev entrypoint): it calls the real
   `ingest.write` path to emit `Sample`s for `series:cooler.temp` and `series:fryer.state` in workspace
   `kfc`, tagging each series `store:downtown-0421`, `kind:temperature`, `equipment:walk-in-cooler`
   (cooler) etc. Real rows, real tags, real caps — no fake.
2. **Open.** Alice (member of `kfc`, holding the dashboard + series read caps) logs in via the browser.
   The shell reads her caps from the session token; the **Dashboards** nav slot is shown (cap-gated). Bob
   in workspace `mcdonalds` sees none of `kfc`'s dashboards or series (the wall, real).
3. **Build.** Alice opens a dashboard (or creates one — `dashboard.save` with a fresh id). She drags a
   **chart** tile onto the grid and binds it to `series:cooler.temp`; a **stat** tile bound to
   `series.latest("cooler.temp")`; a **gauge** bound via a **tag** binding
   `{ find: { tags: ["kind:temperature", "store:downtown-0421"] } }` (resolved by `series.find`). Each
   resize/move persists the layout via `dashboard.save` (UPSERT).
4. **Live.** Each widget backfills history with `series.read` (chart) / `series.latest` (stat, gauge),
   then opens `GET /series/cooler.temp/stream` (SSE) and folds live samples into its series — the chart
   line advances, the stat re-renders. Motion over the bus, not a poll.
5. **Isolation.** Carol (a second `mcdonalds` session) builds her own dashboard; her widget binding to a
   `kfc` series id is **denied** server-side (the series wall), and her `dashboard.list` returns only
   `mcdonalds` dashboards. The two-session isolation test passes on real sessions.
6. **Delete.** Alice deletes a dashboard → `dashboard.delete` tombstones it; `dashboard.list` no longer
   returns it; a re-delete is a no-op (idempotent).

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` (real infra, seeded via the real write path —
**no mock data, no `*.fake.ts`**; the frontend tests drive a real in-process gateway, per STATUS item 00):

- **Capability deny — per verb.** A principal without `mcp:dashboard.get`/`.list`/`.save`/`.delete`/
  `.share` is refused that verb (nothing read/written); a widget reading a series without `mcp:series.read`
  is refused. Deny is opaque. **One deny-test per verb built** (5 dashboard + reuse the series deny tests).
- **Gate-3 membership deny (the S4 sharing test, extended).** A dashboard shared `team` is read by a
  member and **refused for a non-member** (gate 3, below the workspace wall); a `private` dashboard is
  invisible to everyone but its owner; `dashboard.list` returns exactly own + team-shared + workspace.
- **Workspace isolation.** Two real sessions: a ws-B principal cannot get/list/save/delete/share a ws-A
  dashboard; a ws-B widget binding to a ws-A series id is denied; `dashboard.list` is workspace-
  partitioned. Across **store + MCP**, the standard two surfaces.
- **Offline / sync.** The `dashboard:{id}` upsert + tombstone replays idempotently on the channel-sync
  path (a dashboard authored offline merges once on reconnect; a re-delivered save/delete does not
  double-apply). Reuses the §6.8 path — assert idempotency, don't build a new mechanism.

Plus this slice's specific cases:

- **CRUD round-trip (backend).** `save` (create) → `get` returns it → `save` (update, same id) → `get`
  reflects the update → `list` includes it → `delete` → `list` excludes it → re-`delete` is a no-op.
- **Seed integrity.** `seed_iot_demo` writes through the real ingest path and `series.read`/`series.find`
  return the seeded samples with the expected tags (the seed is honest, not a shortcut).
- **Live SSE route.** A posted sample on `ws/kfc/series/cooler.temp` arrives as an SSE `event: sample` on
  `GET /series/cooler.temp/stream`; an unauthenticated `?token=` is `401` before any stream; an ungranted
  session is `403` (mirrors the channel-stream tests).
- **Vitest (frontend).** Against the real in-process gateway seeded with real rows:
  - `DashboardView` renders cells from a fetched layout; drag/resize calls `dashboard.save`.
  - each widget (`chart`/`stat`/`gauge`) backfills from `series.read`/`series.latest` and folds an SSE
    `sample` into its render (live update visible).
  - a tag-bound widget resolves series via `series.find`.
  - the deny + isolation cases at the UI boundary (a ws-B view shows no ws-A dashboards/series).

## Risks & hard problems

- **Live-update fan-out at scale.** A dashboard with many widgets opens many series SSE streams. Phase 1
  bound: one SSE stream **per distinct series** on a dashboard (de-dup bindings client-side), and a tag
  binding subscribes to its resolved set. A **multiplexed series stream** (one SSE connection, many
  series) is the scaling follow-up — name it, don't pre-build it. Watch connection count in the live SSE
  test.
- **Binding to a non-existent / un-granted series.** A widget must degrade honestly: an empty/loading/
  denied cell state, never a fake value (the no-mock rule applies to the UI). The deny is the series
  wall doing its job; render it as a deny, not a blank.
- **Layout record growth.** A dashboard with very many cells grows the record. Bounded by a sane cell cap
  per dashboard (config); `dashboard.list` returns metadata only (no cell bodies) so the roster stays
  cheap. Stated, not deferred-silently.
- **The Phase-2 federation trust surface (the load-bearing future risk).** A federated widget is the
  weakest principal — read-only series in one cell, host-mediated bridge, no token. The Phase-1 binding
  contract must already be exactly that narrow so Phase 2 is a renderer swap, not a contract redesign.
  The risk is letting a Phase-1 widget reach beyond the three read verbs; the cell host must forward
  *only* the binding's reads even first-party (defense-in-depth dress rehearsal for the bridge).
- **Tag-binding cardinality.** A `find` binding over a broad facet could resolve to a huge series set.
  Bound the resolved set (a cap), and let the tag-cardinality cap (`scope/tags/`) bound the dimensions.

## Open questions

Decisions are **made** below so Phase 1 codes with no open question (HOW-TO-CODE §3 step 4a); the residual
opens are explicitly Phase-2/3 or named follow-ups, not Phase-1 gaps.

**Resolved for Phase 1 (decisions taken):**

- **Grid library:** `react-grid-layout` (named in the plan; mature, draggable/resizable, controlled
  layout that maps 1:1 to the `cells[]` record). Accepted.
- **Layout persistence:** SurrealDB `dashboard:{id}` record, **not** `localStorage` (rule 4). Decided.
- **Widget binding shape:** `{ series } | { find: { tags } }` — explicit series **or** tag-facet query
  via the shipped `series.find`. Both, decided (covers the single-series and the discovery cases).
- **Live transport:** a new `GET /series/{series}/stream` SSE route over the existing Zenoh series
  motion, **not** polling. Decided (mirrors the channel stream).
- **`save` semantics:** one idempotent UPSERT verb for create+update (fresh id creates, existing id
  updates) — not separate create/update. Decided.
- **Built-in widget set v1:** `chart` (time-series line), `stat` (single value), `gauge`. Decided; more
  widget types are additive (and, post-Phase-2, installable).

**Named follow-ups (out of Phase 1, not silent gaps):**

- **Per-dashboard / team-scoped sharing** — Phase 1.5; the `lb-authz` grant primitives exist, the wiring
  (a `sub`-style grant per dashboard or a team-owned dashboard) is deferred. Workspace + cap is the
  Phase-1 boundary.
- **Multi-admin live layout refresh** — a layout edited by admin A appearing live for admin B; same class
  as the admin console's open item. Phase 1 is single-editor.
- **Multiplexed series stream** — one SSE connection for many series (the fan-out scaling fix).
- **Dashboard table into the sync set** — the one-line addition to the §6.8 sync path (carry-over class).

**Phase 2 — now its own build-ready scope: `dashboard-widgets-scope.md`** (widgets as installed
extensions). That doc specifies, with no open questions, **how a widget extension accesses data** (it
never touches the DB or holds the token — host-mediated read-only bridge, ws+cap re-checked per call),
the two trust tiers (trusted module-federation in-process vs untrusted iframe sandbox), the `[widget]`
manifest, the widget palette, and the access model. **Both forever-contracts (the `[widget]` manifest
block + the bridge protocol) are frozen in that doc** — Phase 2 has no open question left; the
stop-and-confirm gate is discharged in the scope, versioned (`v:1`) so future growth is additive.

**Phase 3 opens:** owned by `node-connection-scope.md`, `fleet-presence-scope.md`, `authz-grants-scope.md`
(grant-by-tag) — written, unbuilt; not this scope's to resolve.

## Build steps (Phase 1, vertical slice order)

Each is a shippable sub-slice; build top-to-bottom (store → cap → MCP → gateway → UI), test as you go.

1. **Seed fixture** — `seed_iot_demo` entrypoint emitting real `Sample`s via the real `ingest.write`
   path, tagged; a test asserting `series.read`/`series.find` return them. (Backend; reuses S8 ingest.)
2. **`dashboard` host service** — `crates/host/src/dashboard/{mod,authorize,get,list,save,delete,share,
   tool}.rs` (one verb per file, FILE-LAYOUT), the `dashboard` table (+ `owner`/`visibility`), the five
   caps, `share` reusing the **S4 `share_doc` edge + three-gate read check**, `call_dashboard_tool` MCP
   bridge. Backend tests: CRUD round-trip + deny-per-verb + **gate-3 non-member deny** + two-ws isolation.
   (Backend.)
3. **Gateway routes** — `role/gateway/src/routes/dashboard.rs` (`GET /dashboards`, `GET /dashboards/{id}`,
   `POST /dashboards`, `DELETE /dashboards/{id}`, `POST /dashboards/{id}/share`), each re-checking the cap
   server-side, ws+principal from the token. Gateway tests mirror the host (deny + gate-3 + isolation +
   round-trip over a real socket). (Backend.)
4. **Series live SSE route** — `role/gateway/src/routes/series_stream.rs` (`GET /series/{series}/stream`),
   the series analog of `stream.rs`. Test: live sample arrives, `401`/`403` paths. (Backend.)
5. **UI client + grid host** — `ui/src/lib/dashboard/{dashboard.api,dashboard.types}.ts` (verbs mirror
   1:1), `ui/src/features/dashboard/{DashboardView,WidgetHost,useDashboard}.tsx` + the grid. Cap-gated
   nav slot. (Frontend.)
6. **Built-in widgets** — `ui/src/features/dashboard/widgets/{ChartWidget,StatWidget,GaugeWidget}.tsx`
   + `useSeries.ts` (backfill via `series.read`/`series.latest`, live via the series SSE). Vitest per
   widget + the view, on the real in-process gateway seeded with real rows. (Frontend.)

Then: session doc, debug entries if anything broke, promote to `public/frontend/dashboard.md`, update
this scope's opens, move STATUS.

## Related

- `vision/0003-iot-dashboard.md` — the worked example this builds (Phase 1 = §3 steps 6/7 over seeded
  data; Phase 3 = §3 steps 1–5/9 + §6 fleet scale). **§4** is the "one app, two deliveries" key idea.
- `scope/ingest/ingest-scope.md` — the shipped `Sample` envelope + `series.read`/`series.latest`/
  `series.find` the widgets read; the seed writes through its `ingest.write` path.
- `scope/tags/tags-scope.md` — series discovery via faceted `series.find` (the tag binding).
- `scope/frontend/dashboard-widgets-scope.md` — **Phase 2** (build-ready): widgets as installed
  extensions — how a widget accesses data (host-mediated read-only bridge, no token, ws+cap re-checked),
  trust tiers, `[widget]` manifest, palette, access model.
- `scope/extensions/ui-federation-scope.md` — the general page bridge Phase 2 **narrows to a widget**;
  the first consumer of that bridge.
- `scope/files/files-scope.md` — the shipped S4 asset sharing (`share_doc`/membership three-gate) the
  dashboard sharing model reuses wholesale.
- `scope/node-roles/node-connection-scope.md` + `fleet-presence-scope.md` — **Phase 3**: real producers
  + the online roster that replaces the seed.
- `scope/auth-caps/authz-grants-scope.md` — **Phase 3** grant-by-tag ("operators see `region:emea`") and
  the **Phase-1.5** per-dashboard sharing primitives.
- `scope/frontend/frontend-scope.md` + `collaboration-scope.md` + `admin-console-scope.md` — the shell,
  the real-session identity, and the cap-gated-nav pattern this surface plugs into.
- `scope/sync/sync-scope.md` — the §6.8 `(table,id)` upsert path the dashboard record rides.
- README **§6.1** (timeseries), **§6.11** (tags), **§6.12/§6.13** (one-app-two-deliveries + extension
  UIs), **§3** (the non-negotiables — state vs motion, one datastore, symmetric nodes, workspace wall).
