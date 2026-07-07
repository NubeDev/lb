# Frontend dashboard scope — Grafana-compatible visualization (the `viz` layer)

Status: **scope (the ask)** — the umbrella for the Grafana-parity visualization slice. Promotes to
[`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md) as each part ships. Target
stage: **S9+ collaboration UI** — additive over the **shipped** v2 widget contract
([`../widget-builder-scope.md`](../widget-builder-scope.md)), the shipped `store.query`/`store.schema`
SQL source, the shipped vars library, and the shipped federation `datasource.*` plane
([`../../../datasources/datasources-scope.md`](../../../datasources/datasources-scope.md)).

We want our dashboards to be **as capable as Grafana, by adopting Grafana's model rather than inventing a
new one** — the same standard chart types, the same per-field option taxonomy (units, decimals,
thresholds, value mappings, color), the same transformation pipeline, the same datasource abstraction, and
the same dashboard JSON so a user can **export a dashboard from Grafana and import it here** (and back).
The presentation half (units, numbers, dates) resolves through the **user-prefs** boundary
([`../../../prefs/user-prefs-scope.md`](../../../prefs/user-prefs-scope.md)) so canonical data renders in
each viewer's locale — Grafana's `fieldConfig.unit` becomes *our* `format.quantity`/`format.datetime`
call, not a second formatting stack. Charts stop being native-SurrealDB-only: a panel queries **any**
registered datasource (native SurrealDB, series, or a federation extension source) the same way.

This README is the **map**. Each concern is its own scope doc in this directory (the user asked for one
file per key part); read this first, then the part you're building.

---

## The reconciliation (the one decision everything hangs on)

We already have a durable **cell** record and a **v2 contract** (`a view bound to an MCP tool call`,
[`../widget-builder-scope.md`](../widget-builder-scope.md)). Grafana has a **panel** model (`type`,
`datasource` + `targets`, `fieldConfig`, `options`, `transformations`, `gridPos`). These are the **same
shape at different altitudes** — we adopt Grafana's taxonomy as an **additive superset of our cell**, and
keep our **MCP-tool-as-datasource spine**. The map (detailed in
[`panel-model-scope.md`](panel-model-scope.md)):

| Grafana panel field | Our cell field | Note |
|---|---|---|
| `type` (`timeseries`, `barchart`, `gauge`, `stat`, `table`, `piechart`, …) | `view` | Adopt Grafana's panel-type ids as our view vocabulary (expand `chart`→`timeseries`/`barchart`/…). [`chart-types-scope.md`](chart-types-scope.md) |
| `datasource` + `targets[]` (a query per datasource) | `source { tool, args }` → generalized to `sources[]` (targets) over a **datasource ref** | A target is an MCP tool call; the datasource picks the tool. [`datasource-binding-scope.md`](datasource-binding-scope.md) |
| `transformations[]` | `transformations[]` (**new**, additive) | A **backend** pipeline (`lb-viz`) the `viz.query` verb runs over the target rows — one impl for every client. [`transformations-scope.md`](transformations-scope.md) |
| `fieldConfig { defaults, overrides[] }` | `fieldConfig` (**new**, additive) | Unit/decimals/min-max/thresholds/mappings/color — **units render via user-prefs**. [`field-config-scope.md`](field-config-scope.md) |
| `options` (per-viz) | `options` (structured per `view`) | Legend/tooltip/orientation/stacking/etc. [`chart-types-scope.md`](chart-types-scope.md) |
| `gridPos { x,y,w,h }` | `x,y,w,h` | Already aligned; Grafana is a 24-col grid (pin our grid to 24). |
| `schemaVersion` + whole-dashboard JSON | import/export mapper | We store our native cell; **Grafana JSON is an interchange format** with a bidirectional mapper at the boundary. [`import-export-scope.md`](import-export-scope.md) |

**Why a mapper, not Grafana-native storage.** We keep persisting our own `Cell`/`Dashboard` record (one
datastore, the shipped v2 contract, no fork). Grafana JSON is translated **at the import/export edge** by
one mapper file — we get interop without two persistence models and without breaking a v1/v2 cell. The
alternative (store Grafana's JSON verbatim) was rejected: it forks our record, bypasses our serde-default
additive-`v` discipline, and couples our store to Grafana's `schemaVersion` churn. We adopt the
*taxonomy and option shapes* (so the map is 1:1) but own the *record*.

## The sub-scopes (one file per part)

1. [`panel-model-scope.md`](panel-model-scope.md) — **the spine.** The additive `Cell`/`Panel` shape:
   `view` (Grafana panel types), `sources[]` (targets over a datasource ref), `fieldConfig`,
   `transformations[]`, structured `options`, the 24-col grid, `schemaVersion`, and how it all stays
   serde-default additive over the shipped v2 cell. Everything else references this vocabulary.
2. [`chart-types-scope.md`](chart-types-scope.md) — **the standard visualization set.** Which Grafana
   panel types we adopt and in what order, each one's renderer (recharts/visx) and `options` shape, and
   the result-shape↔type validation. "Start with one chart" = `timeseries` end to end.
3. [`field-config-scope.md`](field-config-scope.md) — **chart options done right** (the user's "options
   are really bad"). The standardized `fieldConfig` (defaults + per-field overrides + matchers): unit,
   decimals, min/max, thresholds, value mappings, color modes, displayName, noValue — **with the
   user-prefs formatting bridge** (`format.*`/`convert.*`).
4. [`transformations-scope.md`](transformations-scope.md) — **the transformation pipeline, backend-resolved.**
   Grafana's transformer set (reduce, organize, filterByName, groupBy, joinByField, calculateField, sortBy,
   limit, merge, …) in a pure Rust `lb-viz` lib behind a **`viz.query(panel) -> { frames }`** verb that
   dispatches the targets and applies the pipeline server-side, returning **canonical** frames — so a React
   Native app, an email render, and the web shell all get identical data with **zero** re-implementation
   (the same doctrine as `format.*`). Bound: heavy aggregation pushes to the query/a job.
5. [`datasource-binding-scope.md`](datasource-binding-scope.md) — **datasources beyond native SurrealDB.**
   The `DataSourceRef` model mapping to native (`store.query`/`series.*`), registered federation sources
   (`datasource:{ws}:{name}` → `federation.query`), and extension tools; the source picker's datasource
   dropdown; leashed by caps + the workspace wall.
6. [`import-export-scope.md`](import-export-scope.md) — **Grafana JSON in/out** (the user's "export from
   Grafana and import here"). The bidirectional mapper, `schemaVersion` migration of older dashboards,
   datasource remapping on import, honest degradation of unsupported types, and the paste/upload/download
   UI.
7. [`panel-editor-scope.md`](panel-editor-scope.md) — **the editor UX** (the user's "ugly" + "edit
   doesn't show the same options as add"). A Grafana-style panel editor (viz picker + Query / Transform /
   Panel options / Field / Overrides tabs) with live preview, built on shadcn + `ui-standards`, with
   **one** field-code path so add and edit can never drift.
8. [`panel-wizard-scope.md`](panel-wizard-scope.md) — **the create-flow wizard** (the user's "Field is
   overwhelming and half of it does nothing"): a stepped wizard (source → chart type → small option
   sections) whose headline is **preview-per-option** — each option card carries a live mini-preview of its
   effect, so live options show their effect instantly and dead options surface themselves. A thin shell
   over the SAME `cellEditorState`/`writeOption`/`usePanelData` engine as the editor (no-drift by
   construction); the simplified-sections + preview engine later **ports back into the editor's Field tab**
   to replace its dead-option rows. Isolated from data studio until validated; real seeded previews (no
   fakes). Backed by the [`fieldTabBaseline`](../../../../../../ui/src/features/dashboard/views/fieldTabBaseline.gateway.test.tsx)
   LIVE/DEAD contract.

## Goals (the umbrella)

- **Adopt Grafana's model, don't reinvent.** Panel types, `fieldConfig`, transformations, datasource ref,
  and dashboard JSON shapes are taken from Grafana (`/tmp/grafana`, the cloned reference) so import/export
  is a 1:1 map and a user's Grafana muscle-memory transfers.
- **A standard, complete chart set** with the full Grafana option surface (legend, tooltip, axes,
  thresholds, mappings, color, stacking, …) — the same options whether **adding or editing** a panel.
- **Canonical-in, localized-out presentation.** Units, numbers, and dates render through user-prefs
  (`format.*`/`convert.*`), never a parallel formatter and never a stored formatted string.
- **Any datasource, one way.** Native SurrealDB, series, and federation extension sources are all
  datasources a target queries; charts are not native-only.
- **Grafana dashboard JSON import/export**, with `schemaVersion` migration and honest degradation.
- **Additive over the shipped contract.** No break to v1/v2 cells; every new field is serde-default; the
  `v`/`schemaVersion` discipline holds. No new datastore, no `if cloud`, no `*.fake.ts`.

## Non-goals (the umbrella; each sub-scope refines its own)

- **Not Grafana the product, nor its plugin runtime, nor Angular panels.** We adopt the *data model and
  option taxonomy*, not Grafana's React/Angular plugin SDK or its server. Unsupported panel types degrade
  honestly on import (named, not faked).
- **No second formatting stack.** Formatting is the user-prefs `format.*`/`convert.*` boundary; this slice
  *consumes* it, it does not re-implement CLDR/unit math.
- **No raw datasource handle at the panel.** A target is always a host-gated MCP tool call (the federation
  doctrine + the v2 bridge leash); a panel never holds a DB connection or a token.
- **No alerting engine here.** Thresholds *color* a value; Grafana-style alert *rules* are the rules
  engine's job ([`../../../rules/rules-engine-scope.md`](../../../rules/rules-engine-scope.md)), not this
  visualization scope.

## Phasing — "start with one chart"

The user's instinct is right: prove the spine on **one** chart before fanning out.

- **Phase 1 — `timeseries` end to end. ✅ SHIPPED (2026-06-29).** The panel-model spine
  (`view:"timeseries"`, a `fieldConfig` with unit/decimals/thresholds rendered through the one user-prefs
  bridge + its documented fallback, the editor tabs, a real source). One chart, the full option surface,
  add==edit (the pinned `cell ↔ editorState` round-trip). ([`panel-model`](panel-model-scope.md) +
  [`field-config`](field-config-scope.md) + [`panel-editor`](panel-editor-scope.md) + the `timeseries` row
  of [`chart-types`](chart-types-scope.md).) Promoted to
  [`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md); session:
  [`dashboard-viz-phase1`](../../../../sessions/frontend/dashboard-viz-phase1-session.md).
- **Phase 2 — the rest of the everyday set. ✅ SHIPPED (2026-06-29).** `stat`, `gauge`, `bargauge`,
  `table`, `barchart`, `piechart` on the same spine — one renderer + typed per-viz `options`
  (Grafana-verbatim) per view, the shared `reduceOptions` frame→value bridge for the single-stat family,
  the fieldConfig render path through the one user-prefs bridge, the editor extended (viewOptions +
  shape-filtered VizPicker + per-view PanelOptions editors), and result-shape↔type validation. No backend
  change, no client transform (invariant B), all data through the one hook (invariant A). Promoted to
  [`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md); session:
  [`dashboard-viz-phase2`](../../../../sessions/frontend/dashboard-viz-phase2-session.md). The remaining
  panels (`histogram`, `state-timeline`/`status-history`, `heatmap`, `text` — the visx/markdown family)
  move to Phase 3. ([`chart-types`](chart-types-scope.md).)
- **Phase 3 — backend resolve (`viz.query` + `lb-viz`) + multi-datasource targets. ✅ SHIPPED
  (2026-06-29).** The transformation pipeline as a host verb (`viz.query(panel) -> {frames, rows}`, gated
  `mcp:viz.query:call`, dispatching each target under `caller ∩ grant` by re-entering the host dispatcher)
  + the pure `lb-viz` crate (Grafana's transformer set, one per file, verbatim ids/options) + the
  datasource dropdown + a real Transform-tab pipeline editor. The one-file client swap landed in
  `usePanelData` (`builder/useVizQuery.ts`); invariants A (one hook) + B (backend-only pipeline) held.
  ([`transformations`](transformations-scope.md) + [`datasource-binding`](datasource-binding-scope.md).)
  Promoted to [`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md); session:
  [`dashboard-viz-phase3`](../../../../sessions/frontend/dashboard-viz-phase3-session.md). Named
  follow-ups (deferred, not silent): `viz.stream` (live frames), `federation.datasource.schema` (federation
  SQL-builder dropdowns), the `format.ts`→`format.*` swap. Phases 1–2 kept the shipped client fetch
  **behind the one data hook** so this swap was confined to one file.
- **Phase 3.5 — editor parity (the usability gap). ✅ SHIPPED (2026-07-03).** A hands-on review found the
  shipped editor was a stub over the (correct) spine: `organize` and 6 other transforms edited through a
  raw-JSON textarea, overrides took free-typed property ids, value mappings/color schemes existed in the
  render path but had no editor, per-viz options covered ~20% of Grafana's surface, and the Query tab was
  single-target despite the `targets[]` model. All 7 steps shipped (primitives → option registry → typed
  transform editors → overrides pickers → per-viz parity → multi-target → per-step debug), each green,
  one commit per step, scoped + logged in [`editor-parity-scope.md`](editor-parity-scope.md) +
  [`../../../../sessions/frontend/dashboard-editor-parity-build-session.md`](../../../../sessions/frontend/dashboard-editor-parity-build-session.md).
  Exit gate MET: **build any supported panel without ever typing JSON or a remembered field name.** The
  one additive backend change was `viz.query`'s opt-in `debug`/`stopAt` per-step frames (no new verb).
- **Phase 4 — Grafana JSON import/export** + `schemaVersion` migration.
  ([`import-export`](import-export-scope.md).)

Each phase writes its session doc, promotes shipped truth to `public/frontend/dashboard.md`, and keeps the
mandatory deny + workspace-isolation tests green.

## How it fits the core (umbrella; each sub-scope details its own)

- **Tenancy / isolation (rule 6):** the dashboard/cell record stays workspace-namespaced; every target is
  a `bridge.call` that derives the workspace from the token; an imported dashboard's datasource refs
  resolve only within the importer's workspace. The two-session test extends to import (a ws-B import can
  never name a ws-A datasource).
- **Capabilities (rule 5/7):** panel data is resolved by **`viz.query`** (gated `mcp:viz.query:call`),
  which dispatches each target under `caller ∩ grant` (composing the target tool's own cap) — no render-path
  bypass. Import/export get their own verbs + caps (`mcp:dashboard.import:call` / `:export:call`). Deny is
  opaque. Detailed per sub-scope.
- **Placement (rule 1):** one editor, two transports (Tauri `invoke` / gateway SSE+HTTP). No role branch.
- **One datastore / state vs motion:** cells/dashboards are SurrealDB state; live samples are motion over
  the shipped SSE; no new store. Grafana JSON is interchange, not storage.
- **MCP is the contract (rule 7):** datasources are MCP tools; the panel is a generic front-end for them.
  Panel data resolution (`viz.query`) and presentation (`format.*`) are both backend-mediated MCP verbs, so
  the React web shell, a React Native app, a server-rendered email, and a webhook are all thin and identical
  — they re-implement neither the transform pipeline nor the unit/date math.
- **One responsibility per file (FILE-LAYOUT):** the implementation lands one panel type / one transform /
  one mapper-direction / one editor tab per file. Each sub-scope names its files.

## Testing strategy (umbrella)

Per [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — real gateway, real store,
seeded real rows, **no `*.fake.ts`**. Each sub-scope names its cases; the cross-cutting gates:

- **Capability deny** per new verb (import/export), and the existing per-target deny holds.
- **Workspace isolation** — extended to import (datasource refs, panel data) across store + MCP.
- **Round-trip fidelity** — a Grafana dashboard JSON → import → export → JSON is semantically stable for
  the supported subset (the headline import/export test).
- **Formatting correctness** — a canonical value renders per the viewer's resolved prefs (units/number/
  date), proving the user-prefs bridge, not a second formatter.

## Risks & hard problems (umbrella; each sub-scope expands)

- **Scope sprawl.** Grafana is enormous; the phasing + the "supported subset, degrade the rest honestly"
  rule is the guardrail. Each sub-scope states its own bound.
- **The mapper is the interop contract.** Get the Grafana↔cell map right once; `schemaVersion` migration
  and lossy fields are where import silently corrupts. [`import-export`](import-export-scope.md) owns it.
- **fieldConfig overrides + matchers** are deceptively deep (per-field, regex/type matchers). Phase the
  matcher set; start with `byName`/`byType`.
- **The user-prefs bridge must be the only formatter.** A regression that hard-codes a unit string in a
  renderer forks the canonical principle. Lint/test the example path.

## Related

- [`../widget-builder-scope.md`](../widget-builder-scope.md) — the shipped v2 contract this is additive
  over (view↔panel-type, source↔target).
- [`../README.md`](../README.md) — the dashboard scope index (add this `viz/` group to its read order).
- [`../../../prefs/user-prefs-scope.md`](../../../prefs/user-prefs-scope.md) — the formatting boundary
  (`format.*`/`convert.*`) the field-config consumes.
- [`../../../datasources/datasources-scope.md`](../../../datasources/datasources-scope.md) — the
  federation `datasource.*` plane the datasource-binding wires into panels.
- [`../../../rules/rules-engine-scope.md`](../../../rules/rules-engine-scope.md) — where alert *rules*
  live (thresholds here only color).
- The Grafana reference clone at `/tmp/grafana` — `kinds/dashboard/dashboard_kind.cue` (panel/fieldConfig/
  schema), `public/app/plugins/panel/*` (panel types), `packages/grafana-data/src/transformations`
  (transformers), `apps/dashboard/pkg/migration` (`schemaVersion`).
- README **§6.1** (timeseries + API shape), **§6.11** (tags/series discovery), **§6.12–6.13** (UI
  delivery + federation), **§3** (rules 1/2/3/5/6/7).
