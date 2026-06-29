# Viz scope — datasource binding (charts beyond native SurrealDB)

Status: **SHIPPED (2026-06-29)** — Phase 3 of the [`viz/`](README.md) slice. A `DataSourceRef` selects a
target's tool (native `surreal`→`store.query`, `series`→`series.*`, registered `federation`→
`federation.query`); `viz.query` dispatches each through the gated tool under the workspace wall (a ws-B
panel can never resolve a ws-A datasource). The datasource dropdown shipped in the editor's Query tab.
**Deferred (named, not silent):** `federation.datasource.schema` (SQL-builder column dropdowns for an
external source — a federation-plane add); a federation target uses the raw-SQL editor until it lands.
Session [`dashboard-viz-phase3`](../../../../sessions/frontend/dashboard-viz-phase3-session.md). Original
ask below.

Part of the [`viz/`](README.md) slice (sub-scope 5); owns the `DataSourceRef`
→ `(tool, args)` resolution that the spine ([`panel-model-scope.md`](panel-model-scope.md)) only declares
in shape. Promotes to [`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md).

One paragraph: the user's complaint is that "extensions and datasources need to work with the charts; at
the moment it's just the native SurrealDB." The fix is **not a new binding mechanism** — the shipped v2
cell already binds a `view` to an MCP tool call (`source { tool, args }`), so the dashboard is *already a
generic front-end for the MCP tool surface*. A "datasource" is just **which tool a target names**. This
scope (a) defines how a `DataSourceRef` resolves to a concrete `(tool, args)` for each kind — native
`store.query`, the `series.*` plane, a registered `federation.query`, or an extension's own read tool —
and (b) surfaces **datasource selection** in the panel builder (Grafana's datasource picker) so the author
picks a source from a unified list instead of typing a tool name. The workspace wall and per-tool caps are
inherited, not re-invented: a `Target.datasource` resolves only within the caller's workspace, and the
host re-checks the tool's cap on every render call.

## Goals

- **Resolve every `DataSourceRef` to a concrete `(tool, args)`** at the target, with one rule per kind
  (`surreal` / `series` / `federation` / `ext:<id>`). No per-kind special field on the cell — the v2
  `{ tool, args }` target already carries everything; the ref just *selects* it.
- **A datasource dropdown in the source picker**, populated from a **unified list**: built-in "SurrealDB
  (native)" + "Series" + each registered federation source (`datasource.list`, with its `datasource.test`
  green/red badge) + each installed extension's data tools (`ext.list`). Selecting a datasource drives
  *which query editor shows*; the author never types a tool name.
- **Wire the shipped federation plane into cells.** Federation sources are registered today
  ([`../../../datasources/datasources-scope.md`](../../../datasources/datasources-scope.md)) but not
  reachable from a panel — close that gap via `federation.query` targets, with `federation.mirror` offered
  as a "cache into series" alternative.
- **Keep the safety story intact and additive:** workspace-pinned resolution, per-tool caps reused (no new
  render cap), DSN/secret never leaves the server.
- **Multi-datasource panels** — `sources[]` lets one panel mix targets across datasources, merged by a
  transformation (cross-ref [`transformations-scope.md`](transformations-scope.md)).

## Non-goals

- **No new binding contract.** We reuse the v2 `{ tool, args }` target + the `DataSourceRef` from the
  spine. This doc is *resolution + picker UX*, not a shape change (that is panel-model).
- **No raw DB handle at the panel.** Federation is always through the gated `federation.query` verb; a
  panel never holds a DSN, connection, or token (federation doctrine).
- **No second authority.** SurrealDB stays the authority; an external DB is a federated **source**, never a
  second datastore (rule 2). A federation target reads live or mirrors into the series plane — it does not
  become canonical state.
- **No datasource CRUD here.** Add/remove/test of a federation source is the federation plane's
  `datasource.*` admin verbs; this doc *consumes* `datasource.list`/`.test`, it does not re-implement them.

## Intent / approach

A `DataSourceRef { type, uid? }` is a **selector over the MCP tool surface**, resolved to `(tool, args)` at
render time:

| `ref.type` | resolves to tool | args | builder editor |
|---|---|---|---|
| `"surreal"` | `store.query` | `{ sql }` | SQL Builder ⇄ Code (dropdowns from `store.schema`) |
| `"series"` | `series.read` \| `series.latest` \| `series.watch` | `{ series \| tags, … }` | series/tag picker (`series.find`) |
| `"federation"`, `uid:"datasource:{ws}:{name}"` | `federation.query` | `{ source:<name>, sql }` | SQL Builder ⇄ Code (dropdowns from `datasource.schema`) |
| `"ext:<id>"` | the extension's own read tool (from `ext.list`) | per its manifest | the ext's arg form |

The author selects a datasource; the builder maps it to the tool + shows the matching editor, then writes a
plain v2 target. Nothing downstream of the picker knows "what kind." At render the targets are dispatched by
the backend **`viz.query`** resolver ([`transformations-scope.md`](transformations-scope.md)) — it calls
each target tool under `caller ∩ grant` (workspace from the token), then applies the transformation pipeline
and returns canonical frames. (A no-transform Phase-1 panel may still resolve via the shipped client
`bridge.call` behind one data hook until `viz.query` lands in Phase 3 — the resolution *contract* is the
same `(tool, args)` either way.)

**Rejected: a per-datasource-kind special binding on the cell** (e.g. `cell.federationSource`,
`cell.seriesBinding`). That multiplies the contract by the number of kinds, forks the render path, and
breaks rule 7 (MCP is the one contract). Collapsing every kind onto **one `{ tool, args }` target + a
`DataSourceRef`** keeps a single render path and a single leash set.

**Also rejected: giving the panel a raw DB handle / DSN for external sources.** It would be faster to query
but violates the federation doctrine and the workspace wall (a client-side DSN can name any tenant). Always
through the gated, workspace-pinned `federation.query` verb — the secret stays server-side.

## How it fits the core

- **Tenancy / isolation (rule 6 — the headline):** a `Target.datasource` resolves **only within the
  caller's workspace**. `ref.uid` is a `datasource:{ws}:{name}` record; the host pins the workspace from
  the token and `federation.query` is **workspace-PINNED** — the `source` name cannot reach another
  tenant's source. A ws-B cell can never name a ws-A datasource. The unified picker list is itself
  workspace-scoped (`datasource.list`/`ext.list` are ws-walled).
- **Capabilities (rule 5/7):** **additive, no new render cap.** A federation target is leashed by
  `cell.tools ∩ grant`, host-re-checked per call against the *tool's existing cap* —
  `mcp:federation.query:call` for federation, `mcp:store.query:call` for native, `mcp:series.read:call`
  for series, and the extension tool's own cap for `ext:<id>`. Deny is opaque. The datasource picker only
  *offers* sources the caller could resolve; a tampered ref still fails the per-call host check.
- **Placement (rule 1):** `either` — pure resolution + UI; identical on edge and cloud. The federation
  extension is a native Tier-2 sidecar wherever it runs (§6.3); the panel doesn't know or branch.
- **MCP surface (§6.1):** the render path dispatches targets through the **`viz.query`** verb
  ([`transformations-scope.md`](transformations-scope.md)) — this binding doc adds no datasource verb of its
  own beyond the federation builder-schema read (next bullet, on the federation plane). Each target stays a
  bounded call to an already-registered tool, leashed by its own cap inside the resolver.
- **Data (SurrealDB):** the cell stores only the `DataSourceRef` + `{ tool, args }` — no rows, no DSN. A
  `federation.query` is read-first, SELECT-only validated, row-capped (the shipped 10k/5s class); native
  `store.query` keeps its parse-allowlist. A "mirror to series" target writes through the shipped
  `federation.mirror` job into the series plane (state stays SurrealDB-owned).
- **Bus (Zenoh):** `series.watch`/`bus.watch` targets stream live over the shipped SSE; federation is
  request/response (no live federation stream in Phase 3 — mirror-then-watch is the live path). State vs
  motion holds.
- **Sync / authority:** the ref is part of the §6.8 cell UPSERT and replays idempotently; an external
  source is a *source*, never authority — a mirror lands in the series plane on the normal sync path.
- **Secrets:** the federation DSN/secret is resolved server-side from the `datasource:{ws}:{name}` record's
  secret ref ([`../../../secrets/secrets-scope.md`](../../../secrets/secrets-scope.md)); it **never**
  reaches the cell, the args, or the result. The picker shows a name + a test badge, nothing more.
- **SDK/WIT impact:** none beyond the spine's v2→v3 cell. An extension exposing a data tool just declares it
  in its manifest; it appears in the picker via `ext.list` with no dashboard-side code.

### One small new verb (decided: yes, on the federation plane)

The visual SQL builder needs column/table dropdowns for an external source — the federation analog of the
shipped `store.schema`. Lean: **add a dedicated read verb** `federation.datasource.schema { source } ->
{ tables:[{ name, columns:[{ name, type }] }] }`, **workspace-pinned**, mirroring `store.schema`, gated
`mcp:datasource.schema:call`. Rejected folding it into `datasource.test`/`datasource.list` (test is a
connectivity probe; list is admin metadata — a schema read is a distinct, cacheable concern). This is an
**addition to the federation plane** (cross-ref [`../../../datasources/datasources-scope.md`](../../../datasources/datasources-scope.md)),
not the dashboard; the builder just calls it like any other read.

## Example flow

1. An author opens the panel editor on a `timeseries` cell and clicks the **datasource dropdown**. The
   builder calls `datasource.list` + `ext.list` (both ws-walled) and shows: **SurrealDB (native)**,
   **Series**, **timescale** (federation, green badge from `datasource.test`), **mysql-prod** (federation,
   red badge), **mqtt** (extension data tool).
2. The author picks **timescale**. The builder records `datasource:{type:"federation",
   uid:"datasource:kfc:timescale"}`, calls `federation.datasource.schema { source:"timescale" }`, and
   populates the SQL Builder's table/column dropdowns — no DSN ever returned.
3. The author builds a SELECT; the builder writes target **A** = `{ refId:"A", datasource:{…}, tool:
   "federation.query", args:{ source:"timescale", sql } }`. Live preview runs one `bridge.call` →
   host pins ws=kfc, checks `mcp:federation.query:call ∈ cell.tools ∩ grant`, resolves the secret
   server-side, returns `{ columns, rows }` (row-capped). The DSN is never on the wire.
4. The author adds target **B** against **Series** (`series.read`) and a `merge`/`joinByField`
   transformation aligns the federation table with the native series in one panel
   ([`transformations-scope.md`](transformations-scope.md)). Each target is independently capped.
5. Because this dashboard is hot and offline-capable, the author clicks **"Mirror to series"** — the
   builder enqueues the shipped `federation.mirror` job for the range, and switches target A to a
   `series.read` over the mirrored series (fast, offline, joinable). Live federation stays available for
   fresh/interactive views.
6. A ws-B viewer opens a copy of the dashboard: `datasource:{ws}:timescale` resolves against **ws-B's**
   registry; if ws-B has no `timescale` source, the target fails the host check (opaque) — it can never
   reach ws-A's source.

## Testing plan

Per [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — real gateway, real store,
seeded real rows, **no `*.fake.ts`**. The one sanctioned fake-boundary is the external DB *driver*; for
federation tests use a **real spawned container** (a Postgres/Timescale or MySQL in CI), not a hand-written
re-implementation.

- **Resolution per kind:** a target with each `DataSourceRef.type` resolves to the expected `(tool, args)`
  and renders real rows — `surreal`→`store.query`, `series`→`series.read`, `federation`→`federation.query`
  (against the spawned container), `ext:<id>`→the ext's read tool (a real seeded extension).
- **Workspace isolation (the headline, mandatory):** seed `datasource:wsA:timescale` and
  `datasource:wsB:timescale` (different endpoints, both real). A ws-A cell resolves ws-A's source; a ws-B
  token resolving `uid:"datasource:wsA:timescale"` is host-pinned to ws-B and **fails** — never reaches
  ws-A. Assert across both store and MCP.
- **Capability deny (mandatory):** a grant lacking `mcp:federation.query:call` (or `mcp:store.query:call`,
  or the ext tool's cap) yields an opaque deny at render; the picker offering the source does not bypass
  the per-call check.
- **Secret never leaks:** assert the DSN/secret appears in neither the saved cell, the `args`, nor the
  `{ columns, rows }` result for a `federation.query` target.
- **The new schema verb:** `federation.datasource.schema` returns the spawned container's real
  tables/columns, is workspace-pinned, and is gated (`mcp:datasource.schema:call` deny tested).
- **Mirror path:** "Mirror to series" enqueues `federation.mirror`; after the job, a `series.read` target
  returns the mirrored rows from the real series plane.
- **Multi-datasource merge:** a panel with a federation target + a series target merged by `joinByField`
  produces the joined frame; each target is independently row-capped (assert the bound).

## Risks & hard problems

- **Datasource availability vs the saved ref.** A ref can outlive its source (source removed, or imported
  into a workspace lacking it). The render must fail *honestly and opaquely* (named "datasource not
  available"), never silently fall back to another source or leak which sources exist. The picker's
  test-badge is advisory; the host check is authoritative.
- **Live federation latency.** `federation.query` is request/response over an external DB — slow or large
  queries hurt interactivity. Mitigation: the row cap + the "mirror to series" escape hatch (surfaced as a
  builder action, **not** a magic default — the author chooses fresh-vs-fast).
- **Schema-cache staleness.** `federation.datasource.schema` dropdowns can drift from the live DB; cache
  with a refresh action, and let `federation.query` be the source of truth (a bad column fails at query).
- **Extension tool arg shape.** An `ext:<id>` data tool's args are manifest-defined and varied; the builder
  must render a generic arg form from the manifest (no per-extension dashboard code) or honestly fall back
  to a raw-args editor for tools without a declared arg schema.
- **No live federation stream.** Federation has no `watch`; a dashboard needing live external data must
  mirror-then-watch. State this clearly so authors don't expect a live federation refresh.

## Resolved decisions

No blocking open questions — these are the long-term answers the build follows.

- **A dedicated `federation.datasource.schema` verb** (not folded into `datasource.test`): test is
  connectivity, schema is structure; separating keeps each bounded and cacheable. It's an addition to the
  federation plane (workspace-pinned, gated) — flag it for the federation-plane owner when Phase 3 lands.
- **One `"federation"` `DataSourceRef.type` + a `uid`**, mirroring Grafana's `{ type, uid }` and keeping
  the type set closed — the source name lives in `uid` (a `datasource:{ws}:{name}` record), never in `type`.
- **Extension data tools stay `ext:<id>` targets surfaced in the same picker** (one dropdown, one
  principle) — no `datasource`-shaped wrapper record unless an extension later wants a `datasource.test`-style
  connectivity badge.
- **"Mirror to series" is an explicit builder action**, offered when a federation target is added to a
  *saved* dashboard (not in preview), never auto-mirrored — authority and freshness are the author's call.

## Related

- [`README.md`](README.md) — the viz umbrella (the reconciliation table; sub-scope 5).
- [`panel-model-scope.md`](panel-model-scope.md) — the spine; owns the `DataSourceRef`/`Target` *shape*
  this doc *resolves*.
- [`chart-types-scope.md`](chart-types-scope.md) (result-shape↔view) · [`field-config-scope.md`](field-config-scope.md)
  (presentation) · [`transformations-scope.md`](transformations-scope.md) (`merge`/`joinByField` for
  multi-datasource panels) · [`import-export-scope.md`](import-export-scope.md) (datasource remapping on
  Grafana import) · [`panel-editor-scope.md`](panel-editor-scope.md) (the picker tab in the editor).
- [`../widget-builder-scope.md`](../widget-builder-scope.md) — the shipped v2 `{ tool, args }` source +
  source-picker principle this builds on.
- [`../../../datasources/datasources-scope.md`](../../../datasources/datasources-scope.md) — the federation
  plane (`datasource.add/list/remove/test`, `federation.query`, `federation.mirror`) this wires into cells,
  and where the new `federation.datasource.schema` verb lands.
- [`../../../rules/rules-engine-scope.md`](../../../rules/rules-engine-scope.md) (rules consume the same
  datasources) · [`../../../ingest/ingest-scope.md`](../../../ingest/ingest-scope.md) (how rows reach the
  series plane) · [`../../../secrets/secrets-scope.md`](../../../secrets/secrets-scope.md) (DSN/secret
  refs) · [`../../../jobs/jobs-scope.md`](../../../jobs/jobs-scope.md) (`federation.mirror` is a job) ·
  [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) (real spawned container).
- The Grafana reference clone at `/tmp/grafana` — the `{ type, uid }` `DataSourceRef` + datasource-picker
  model we align to.
- README **§6.1** (API shape), **§6.3** (two tiers — federation is Tier-2), **§3** (rules 2/5/6/7).
