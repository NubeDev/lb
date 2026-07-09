# Dashboard scope — Grafana → our-dashboard conversion (standalone tool: JSON-in → JSON-out)

Status: **scope (the ask)**. The near-term deliverable is a **standalone conversion tool** — a small
**Rust backend + shadcn UI + Tauri** app (browser + Linux/Windows) that takes a **Grafana dashboard JSON
file in and emits our dashboard JSON out** (one direction: Grafana → us). It lives in the repo as its own
mini-project (own cargo workspace, own UI) rather than wired into the host node yet — the user will fold it
into the main project later. The audit below stays the design substrate; promotes to
[`public/dashboard/dashboard.md`](../../../../doc-site/content/public/dashboard/dashboard.md) as it ships.
Aligned with the shipped v3 panel model ([`viz/panel-model-scope.md`](viz/panel-model-scope.md)) and the
shipped vars library ([`widget-config-vars-scope.md`](widget-config-vars-scope.md)) — it emits **their**
record shape.

One paragraph: the user wants to **feed a Grafana dashboard JSON and get our dashboard JSON back out**,
runnable as its own tool right now (paste/drop a `.json`, get the converted `.json`), and to fold it into
the main project later. So the immediate build is a **one-way mapper** — Grafana classic `panels[]` +
`templating.list[]` → our `Dashboard` record (`cells[]` + `variables[]`, the shapes in
[`rust/crates/host/src/dashboard/model.rs`](../../../../rust/crates/host/src/dashboard/model.rs)) — behind
a tiny app: a Rust conversion crate exposed over a thin local HTTP/Tauri-command seam, and a shadcn UI that
takes the input JSON, shows the converted output + an honest "what degraded / dropped" report, and lets you
copy/download it. Export back to Grafana, `dashboard.import` as a host verb, and datasource remapping are
**out of this cut** and stay in [`viz/import-export-scope.md`](viz/import-export-scope.md). The audit
(below) is what the mapper's "preserved + flagged" report is built from.

## Goals

- **A standalone Grafana→our JSON converter that runs today.** Rust backend (the mapper crate + a thin
  serve/command seam), shadcn UI, Tauri packaging for browser + Linux/Windows. Input: a Grafana dashboard
  export `.json`. Output: our `Dashboard` JSON (the `model.rs` shape) **plus** a conversion report listing
  every feature that was mapped, degraded (preserved-but-not-rendered), or dropped.
- **Map the common page 1:1; degrade the rest honestly.** Panels/grid, the have-it variable types,
  `time`/`refresh`, tags/title/description map cleanly. Everything the audit marks "close now / degrade /
  out" is *reported*, never silently lost — the report is the tool's headline deliverable.
- **Emit the real record shape, not a fork.** The output is exactly the `Dashboard`/`Cell`/`Variable`
  serde shape the host already persists (imported as source-of-truth from `model.rs`), so folding this into
  the main project later is a wiring job, not a re-map. Rows land as `view:"row"` cells and advanced
  variables as the existing optional `Variable` fields — both already in `model.rs`.

## Non-goals

- **Not the export half.** Our JSON → Grafana JSON is a later direction; this cut is Grafana → us only.
- **Not a host verb (yet).** No `dashboard.import`/`dashboard.export`, no new cap, no `dashboard.save` from
  the tool. It's a standalone file-in/file-out app; the host-verb + workspace-scoped persistence path stays
  in [`viz/import-export-scope.md`](viz/import-export-scope.md) for when this folds in.
- **Not `schemaVersion` migration or datasource remapping.** We read classic schema (42) as-is; older
  schema migration and mapping Grafana datasource UIDs → our federation datasources are the mapper scope's
  problem, not this tool's first cut (both **reported** as degraded where they'd matter).
- **Not Grafana's full dashboard-level surface.** Annotations, dashboard links, adhoc/groupby filter
  variables, panel/row *repeat*, shared-crosshair tooltip, snapshots, fiscal-year/week-start time config —
  each is triaged in the matrix; most **degrade-honestly** or are **out of scope**, and the tool's report
  names each so nothing looks silently dropped.
- **Not the v2 (`dashboard.grafana.app/v2beta1`) kind-based layout.** We target the classic flat `panels[]`
  model (schemaVersion 42); the v2 rows/tabs/auto-grid layout is a future input format we do not accept
  (reported as unsupported if fed one).

## Intent / approach

**A thin standalone app around a pure mapper.** The core is one Rust crate: `grafana_json → Dashboard`,
a pure function over serde values with a sibling `ConversionReport` (mapped / degraded / dropped, each with
a reason). Around it, the smallest possible shell: a Rust seam that serves the mapper (an Axum route for the
browser build, a Tauri command for desktop — the same crate behind both), and a shadcn UI that is one screen
— drop/paste input JSON on the left, converted output + the report on the right, copy/download. The mapper
partitions Grafana's model per the audit's four buckets; the report is that partition made concrete for the
actual input document.

**The output type is imported, not re-declared.** The tool depends on the real `Dashboard`/`Cell`/
`Variable` types (vendored from or path-referenced to `model.rs` — see the standalone-placement note under
"How it fits the core"). It never invents a parallel record shape — that is what keeps "fold it into the
main project later" a wiring task, and what makes the mapper's output provably the shape the host stores.

**Rejected: build a bespoke "Grafana importer" that stores Grafana JSON and renders from it.** That is the
same fork the [viz umbrella](viz/README.md) and the [panel-model spine](viz/panel-model-scope.md) already
rejected — it couples storage to Grafana's schemaVersion churn (42 and climbing) and forks the v3 cell. We
own the record; Grafana JSON is interchange mapped at the edge. The tool's job is to make **our** record
*express* a Grafana page.

**Rejected: build it inside the host node now.** The user explicitly wants a standalone tool first, folded
in later. Wiring a new import verb, cap, and workspace-scoped save into the live node before the mapping is
proven would couple an unproven mapper to the core's auth/tenancy chokepoints — the exact leak rule 10
warns against. A standalone file-in/file-out app proves the mapping against real exports with zero core
surface; the host-verb wiring is a deliberate later step (the mapper scope).

**Rejected: one giant "grafana-parity" scope.** Rows and variables remain independently testable and
independently valuable (the model fields for both are already shipped in `model.rs`); the standalone tool
consumes them. One file per part (FILE-LAYOUT), matching the sibling `viz/` convention.

## Stage 1 — the audit (the gap matrix)

Grafana dashboard-level model, classic schema (`/tmp/grafana/kinds/dashboard/dashboard_kind.cue`,
schemaVersion **42**), triaged. "Have" = already in our record/model; "Close now" = a slice in this
umbrella; "Degrade" = the mapper preserves + flags it, no render (honest placeholder / dropped-with-notice);
"Out" = not carried at all, named here so it is a decision.

### Panels / layout

| Grafana feature | Where (Grafana) | Our state | Fate |
|---|---|---|---|
| Panels (`panels[]`, `type`, `gridPos{x,y,w,h}`) | `#Panel`, 24-col grid | **Have** — v3 `Cell` + 24-col grid | Have |
| **Rows** (`type:"row"`, `collapsed`, nested `panels[]`) | `#RowPanel` `:833-857` | **None** — flat `cells[]`, no grouping | **Close now** → [`panel-rows-scope.md`](panel-rows-scope.md) |
| Library-panel reference (`panel.libraryPanel`) | `#LibraryPanelRef` | **Have** — `panelRef` cell field (library-panels scope) | Have |
| Panel `repeat` (repeat a panel per variable value; `repeatDirection`, `maxPerRow`) | `#Panel.repeat` `:650-659` | None | **Degrade** (named follow-up; depends on multi-value vars — see variables scope) |
| Row `repeat` (repeat a row per variable value) | `#RowPanel.repeat` `:856` | None | **Degrade** (follow-up of the rows slice + multi-value vars) |

### Template variables (`templating.list[]`)

| Grafana feature | Where | Our state | Fate |
|---|---|---|---|
| `query` / `custom` / `constant`(→`const`) / `textbox`(→`text`) / `interval` types | `#VariableType` | **Have** (`query`/`custom`/`const`/`text`/`interval`/`source`) | Have |
| `multi`, `includeAll` | `VariableWithMultiSupport` | **Have** | Have |
| **label ≠ value per option** (`__text`/`__value`, `text : value`, `(?<text>)`/`(?<value>)`) | `#VariableOption` `:235-244` | **None** — `custom?: string[]` is value-only | **Close now** → [`dashboard-variables-advanced-scope.md`](dashboard-variables-advanced-scope.md) |
| **Chained / dependent** variables (a query referencing `$otherVar`) | interpolation-derived, no schema edge | **None** — resolvers resolve independently | **Close now** → variables scope |
| **regex extraction / capture groups** (`regex`, `regexApplyTo`) | `:220-222` | None | **Close now** → variables scope |
| **`sort`** (alpha/numeric asc/desc, natural, case-insensitive) | `#VariableSort` `:271` | None | **Close now** → variables scope |
| **`refresh`** (never / onDashboardLoad / onTimeRangeChanged) | `#VariableRefresh` `:250` | None (we have dashboard-level auto-refresh, not per-var) | **Close now** → variables scope |
| **`allValue`** (custom "All" string) | `:219` | None (we expand All) | **Close now** → variables scope |
| **`hide`** (dontHide / hideLabel / hideVariable) | `#VariableHide` `:254` | Partial (label only) | **Close now** → variables scope |
| **format hints** (`json`/`csv`/`pipe`/`raw`/`singlequote`/`doublequote` have; `regex`/`glob`/`distributed`/`percentencode`/`sqlstring`/… missing) | `VariableFormatID` | Partial (6 of ~20) | **Close now** (extend the set) → variables scope |
| **`datasource` variable type** (pick a datasource by variable) | `DataSourceVariableModel` | None | **Close now** (thin — maps to our federation datasource list) → variables scope |
| `current` selection persistence | `:203` | **Have** (URL-carried selection) | Have (note the encoding difference in the mapper) |
| **adhoc filters** (`adhoc` type — key/op/value chips over a datasource) | `AdHocVariableModel` | None | **Degrade** (needs datasource key discovery; named follow-up) |
| **groupby** variable | `GroupByVariableModel` | None | **Out** (datasource-specific) |
| system vars (`$__dashboard`/`$__org`/`$__user`) | `SystemVariable` | Partial (built-ins: `${__user.login}`/`${__workspace}`/…) | Have (map to our built-ins where present, else degrade) |

### Other dashboard-level features

| Grafana feature | Where | Fate |
|---|---|---|
| `time` / `refresh` / `timepicker` | `:53-73` | **Have** (time range + auto-refresh live in routing) — mapper wires them |
| `timezone`, `weekStart`, `fiscalYearStartMonth` | `:42-70` | **Degrade** (timezone → user-prefs where present; the rest dropped-with-notice) |
| `tags`, `title`, `description` | `:36-39` | **Have** (record fields) |
| `annotations` (event overlays) | `#AnnotationContainer` | **Out** (no annotation plane; named) |
| dashboard `links` | `#DashboardLink` | **Out** (nav owns links; named) |
| `graphTooltip` (shared crosshair) | `#DashboardCursorSync` | **Out** (named) |
| `liveNow`, `preload`, `editable` | `:67`/`:104`/`:45` | **Degrade** (dropped-with-notice) |
| `snapshot` | `#Snapshot` | **Out** (snapshots are a Grafana product feature) |
| `schemaVersion` (42) | `:77` | **Have** (the mapper's migration pin — see import-export scope) |

**The two conversion traps the audit surfaces** (both handled by the "close now" slices, both are why a
naive mapper corrupts a page):
1. **Rows are dual-encoded.** A *collapsed* row nests its children in `row.panels[]`; an *expanded* row has
   empty `panels[]` and its children are flat siblings identified only by grid `y`. The rows slice defines
   which encoding *we* store and the mapper normalizes both to it. (`DashboardModel.ts:956-1035`.)
2. **Chained variables have no explicit edges.** Grafana rebuilds the dependency graph by parsing `$var`
   references out of each variable's query string. The variables slice makes resolution
   dependency-ordered so a `$region`-in-`$host`-query chain resolves in order, not with a literal `$region`.

## How it fits the core

**This cut is a standalone tool — it deliberately touches none of the core seams below yet.** It reads a
file and writes a file; there is no token, no workspace, no cap, no store, no bus in play. The section
records how it *will* fit when folded in, and why the standalone shape doesn't violate the rules meanwhile.

- **Placement — standalone mini-project.** Its own cargo workspace (e.g. `tools/grafana-conv/` with a
  `mapper` crate + a thin `app` binary) and its own shadcn/Tauri UI, outside `rust/crates/*` and outside
  `ui/src`. It is **not** a core crate and **not** the UI shell, so rules 1/5/6/7/10 about core crates do
  not bind it — but it stays honest to them so the fold-in is mechanical.
- **The output type is the real one.** The mapper's return type is the `Dashboard`/`Cell`/`Variable` shape
  from [`rust/crates/host/src/dashboard/model.rs`](../../../../rust/crates/host/src/dashboard/model.rs).
  Because this cut is a *separate workspace* it **vendors** those types (a copied `model.rs`, header-noted
  as a mirror of the host's, kept in sync) rather than path-depending into the host crate — the cleaner
  isolation the standalone choice buys, at the cost of one file to re-sync on fold-in. The fold-in step
  (mapper scope) deletes the mirror and depends on the host type directly.
- **Tenancy / isolation (rule 6):** **not exercised in this cut** — no workspace, no token; the tool maps
  bytes to bytes. When folded in, the workspace comes from the caller's token, never from imported JSON,
  exactly as the dashboard plane does today (owned by [`viz/import-export-scope.md`](viz/import-export-scope.md)).
- **Capabilities (rule 5/7):** **no host verb, no cap** in this cut. The future import path rides
  `mcp:dashboard.save:call`; that wiring is the mapper scope's, not this tool's.
- **Data / Bus:** none — the tool persists nothing (file out); no SurrealDB, no Zenoh.
- **Extensions (rule 10):** the mapper treats every Grafana `panel.type` / datasource / variable as opaque
  data — it never branches on one of *our* extension ids. A Grafana panel type it can't map becomes a
  reported degrade (`view:"template"` placeholder or dropped-with-notice), not a special case.
- **SDK/WIT impact:** none in this cut (standalone). The model fields it emits (`view:"row"` cells, the
  advanced `Variable` fields) are already shipped in `model.rs`; this tool consumes them.
- **Skill doc:** **N/A** — a standalone file-in/file-out desktop/browser tool has no agent-/API-drivable
  host surface. The eventual `dashboard.import` verb's drivable surface belongs to
  [`viz/import-export-scope.md`](viz/import-export-scope.md).

## Phasing

1. **Stage 0 — the standalone converter (this cut, the near-term build).** The `mapper` crate
   (`grafana_json → Dashboard + ConversionReport`), the thin Rust serve/command seam, and the one-screen
   shadcn/Tauri UI (browser + Linux/Windows). Input Grafana `.json` → output our `.json` + report.
   Consumes the audit below as its mapping spec and the already-shipped `model.rs` fields as its target.
2. **Stage 1 — the audit (below). Done in this doc.** The gap matrix + the two traps + the triage — the
   mapping spec Stage 0 implements and the source of the tool's degrade/drop report.
3. **Stage 2 — fold into the main project.** Replace the vendored `model.rs` mirror with a direct host-type
   dependency and wire the mapper behind a real `dashboard.import` verb (cap + workspace-scoped save). This
   is [`viz/import-export-scope.md`](viz/import-export-scope.md)'s job, not re-scoped here.
4. **Stage 3 — the export direction.** Our JSON → Grafana JSON, plus datasource remapping and older
   `schemaVersion` migration — also the mapper scope. Out of the standalone cut.

Panel rows ([`panel-rows-scope.md`](panel-rows-scope.md)) and advanced variables
([`dashboard-variables-advanced-scope.md`](dashboard-variables-advanced-scope.md)) are **already shipped as
model fields** (`view:"row"` cells; the advanced `Variable` fields in `model.rs`); this tool emits them, so
they are inputs here, not phases of it.

## Testing plan

Per [`../../testing/testing-scope.md`](../../testing/testing-scope.md) — **no `*.fake.ts`**, no
re-implemented node behavior. A real Grafana export `.json` is a **fixture**, not a fake backend; the tool
is a pure mapper, so the bulk of the test surface is real-fixture → asserted-output, which the rules
encourage. Because this cut is a standalone file-in/file-out tool with no token/store/cap, the mandatory
capability-deny and workspace-isolation gates **do not apply to this cut** — they attach at the fold-in
(the mapper scope's `dashboard.import` verb), and are named here so they're a decision, not an omission.

- **Golden round-trip (headline):** each real Grafana export fixture maps to an asserted `Dashboard` JSON.
  Rust unit tests over the `mapper` crate: `grafana_json` in → assert `cells[]`/`variables[]` shape out.
  The `Dashboard` the mapper emits must deserialize through the vendored `model.rs` (proves it's the real
  shape) — a `serde_json::from_value::<Dashboard>` on the output is the fold-in guard.
- **Report completeness:** every feature the audit marks degrade/out that appears in a fixture shows up in
  the `ConversionReport` — a dropped feature with no report line is a test failure (the tool's honesty
  contract). Assert the report, not just the output.
- **Row dual-encoding:** a fixture with a *collapsed* row (children nested in `row.panels[]`) and one with
  an *expanded* row (empty `panels[]`, children as flat `y`-ordered siblings) both normalize to the same
  `view:"row"` cell membership (the #1 corruption trap below).
- **Advanced variables:** a fixture exercising label≠value options, regex capture split, sort, and
  `allValue` maps onto the corresponding `Variable` fields (already in `model.rs`).
- **UI (shadcn/Tauri):** a component test over the one screen — paste input JSON, assert the output pane
  and the report render; drives the same `mapper` crate through the serve/command seam (no mock of it).
- **Mirror-sync guard:** a test (or CI check) that the vendored `model.rs` matches the host's, so the
  fold-in never discovers a drifted shape.

## Risks & hard problems

- **Row dual-encoding** (collapsed-nested vs expanded-sibling) is the #1 mapper corruption source — the
  mapper must normalize both Grafana encodings to one `view:"row"` cell membership. Tested with a fixture
  of each (see Testing plan).
- **Model mirror drift.** Vendoring `model.rs` into the standalone workspace buys isolation but risks the
  emitted shape silently diverging from what the host stores — caught by the mirror-sync guard and the
  `from_value::<Dashboard>` round-trip on every mapper output. The fold-in deletes the mirror.
- **Grafana datasource UIDs.** A Grafana target names a datasource by UID; we have no mapping to our
  federation datasources in this cut, so a mapped `sources[]` target carries the UID as opaque data and the
  report flags "datasource unresolved" — honest, not silently broken. Resolution is the fold-in's job.
- **Chained-variable ordering** has no explicit graph in Grafana JSON — derived by parsing `$var` refs; a
  cycle must fail honestly (reported), not hang. The resolution *runtime* is the shipped client resolver;
  this tool only needs to emit variables in a resolvable order (or report an unresolvable cycle).
- **Scope creep into "Grafana the product" — and into the host.** Two guardrails: the matrix's "out /
  degrade" columns (adhoc filters, annotations, links, snapshots are explicitly *not* mapped), and the
  standalone boundary (no host verb/cap/store in this cut — that's Stage 2).

## Open questions

- **Serve seam shape:** does the browser build hit a local Axum route (`POST /convert`) and the desktop
  build a Tauri command, sharing the `mapper` crate — or does the UI call a wasm-compiled `mapper` directly
  (no server at all)? Lean Axum+Tauri command around one native crate first (simplest, one code path per
  target); revisit wasm if a zero-backend browser build is wanted. **Recommend deciding at build time.**
- **Vendor vs path-dep for `model.rs`:** vendored mirror (chosen — matches the standalone-workspace ask,
  one file to re-sync) vs a Cargo path dependency into the host crate (no drift, but drags the host crate's
  deps into the tool). Revisit only if the mirror proves painful to keep in sync.
- **Report surface:** is the `ConversionReport` a flat list of `{feature, fate, reason}` lines, or grouped
  by the audit's four buckets? Lean grouped (matches the matrix the user reasons about). Resolved in the
  build.
- **`datasource` variable / UID resolution** stays deferred to the fold-in (see the datasource risk).

## Related

- [`viz/import-export-scope.md`](viz/import-export-scope.md) — the JSON mapper this umbrella makes ready
  (the conversion tool's actual verbs; Stage 4). · [`viz/README.md`](viz/README.md) — the Grafana-compat
  viz umbrella (panel/fieldConfig/transform/datasource — the *panel* half; this is the *page* half).
- [`panel-rows-scope.md`](panel-rows-scope.md) · [`dashboard-variables-advanced-scope.md`](dashboard-variables-advanced-scope.md) —
  the two "close now" slices.
- [`viz/panel-model-scope.md`](viz/panel-model-scope.md) — the additive-v3 `Cell` these fields extend ·
  [`widget-config-vars-scope.md`](widget-config-vars-scope.md) — the shipped variable system + `vars` lib
  this deepens · [`reusable-pages-scope.md`](reusable-pages-scope.md) — `Variable.required` (page params),
  the precedent for additive variable fields.
- [`native-import-export-scope.md`](native-import-export-scope.md) — the shipped our-format bundle
  (orthogonal: our dashboards between our nodes; this is Grafana→us).
- Grafana reference clone `/tmp/grafana`: `kinds/dashboard/dashboard_kind.cue` (schema, schemaVersion 42),
  `packages/grafana-data/src/types/templateVars.ts` (VariableModel), `public/app/features/dashboard/state/DashboardModel.ts`
  (row collapse/expand + repeat), `public/app/features/templating/formatVariableValue.ts` (format hints).
- Public: [`../../../../doc-site/content/public/dashboard/dashboard.md`](../../../../doc-site/content/public/dashboard/dashboard.md).
- README **§6.1** (API shape), **§3** (rules 1/5/6/7/10).
