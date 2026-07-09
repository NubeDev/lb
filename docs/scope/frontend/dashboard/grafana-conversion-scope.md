# Dashboard scope — Grafana → our-dashboard conversion (the readiness umbrella + Stage-1 audit)

Status: **scope (the ask)**. Umbrella for "**turn a Grafana dashboard page into one of our dashboard
pages**". Promotes to [`public/dashboard/dashboard.md`](../../../../doc-site/content/public/dashboard/dashboard.md)
as each slice ships. Additive over the shipped v3 panel model
([`viz/panel-model-scope.md`](viz/panel-model-scope.md)) and the shipped vars library
([`widget-config-vars-scope.md`](widget-config-vars-scope.md)).

One paragraph: the user wants to **paste a Grafana dashboard and get an equivalent page here**. The
JSON-level mapper that does the actual in/out already has a home —
[`viz/import-export-scope.md`](viz/import-export-scope.md) (`dashboard.import`/`dashboard.export` + a
bidirectional `grafana↔cell` mapper, scoped, Phase 4, **not built**). This umbrella is the **readiness
layer that mapper needs**: first an honest **audit** (Stage 1) of what a Grafana *page* carries that our
model does not, then the two model gaps that audit surfaces as high-value and cheap to close —
**panel rows** ([`panel-rows-scope.md`](panel-rows-scope.md)) and **richer template variables**
([`dashboard-variables-advanced-scope.md`](dashboard-variables-advanced-scope.md)). Everything else the
audit finds is triaged into "close now / degrade honestly / out of scope" so the mapper's honest-degradation
list is a *decision*, not a surprise. No new host verb comes from this umbrella — rows and variables are
**additive record fields** that ride the shipped `dashboard.save`/`dashboard.get` and the shipped
client-side variable resolver; the only new *verbs* are the import/export pair, which stay in their own
scope.

## Goals

- **Stage 1 — the audit is a deliverable.** A concrete gap matrix (below): every Grafana dashboard-level
  feature × {have it | close now | degrade honestly | out of scope}, grounded in the Grafana clone
  (`/tmp/grafana`, `kinds/dashboard/dashboard_kind.cue` schemaVersion 42). This is the input the
  import mapper's "preserved + flagged" list is built from — we decide the triage up front.
- **Close the two named gaps** the audit ranks highest-value / lowest-cost:
  - **Panel rows** — collapsible section headers that group panels (Grafana `type:"row"`). We have no
    grouping primitive today.
  - **Advanced variables** — label≠value options, chained/dependent variables, regex extraction, sort,
    refresh mode, `allValue`, and a wider format-hint set. We have a one-value-per-option model today.
- **Feed, not fork, the JSON mapper.** These slices make the target model rich enough that
  [`viz/import-export-scope.md`](viz/import-export-scope.md) maps a real Grafana page 1:1 for the common
  case, instead of degrading rows and half the variables. The mapper stays the delivery vehicle.

## Non-goals

- **Not re-scoping the JSON mapper.** `dashboard.import`/`dashboard.export`, `schemaVersion` migration, and
  datasource remapping live in [`viz/import-export-scope.md`](viz/import-export-scope.md). This umbrella
  makes the *destination model* ready; it does not move the verbs.
- **Not Grafana's full dashboard-level surface.** Annotations, dashboard links, adhoc/groupby filter
  variables, panel/row *repeat*, shared-crosshair tooltip, snapshots, fiscal-year/week-start time config —
  each is triaged in the matrix; most are **degrade-honestly** or **out of scope**, named so no one thinks
  they were missed.
- **Not the v2 (`dashboard.grafana.app/v2beta1`) kind-based layout.** Exported dashboards today are the
  classic flat `panels[]` model (schemaVersion 42); the v2 rows/tabs/auto-grid layout redesign is a future
  export format we do not target (flagged in the rows scope).

## Intent / approach

**Audit → close the two cheap high-value gaps → hand a rich target to the existing mapper.** The audit
(Stage 1) is the thinking; it partitions Grafana's dashboard-level model into four buckets so every feature
has a decided fate. The two "close now" buckets each get a sub-scope that adds **serde-default additive
fields** to the record we already persist — a row is a `Cell` with `view:"row"`; the variable extensions
are optional fields on the existing `Variable`. Nothing new is stored in a new table, no new host verb, and
the shipped v1/v2/v3 cells and pre-existing variables load unchanged.

**Rejected: build a bespoke "Grafana importer" that stores Grafana JSON and renders from it.** That is the
same fork the [viz umbrella](viz/README.md) and the [panel-model spine](viz/panel-model-scope.md) already
rejected — it couples our store to Grafana's schemaVersion churn (42 and climbing) and forks the v3 cell.
We own the record; Grafana JSON is interchange mapped at the edge. This umbrella's job is to make **our**
record able to *express* a Grafana page, not to adopt Grafana's storage.

**Rejected: one giant "grafana-parity" scope.** Rows and variables are independent, independently testable,
and independently valuable (rows help every native dashboard, not just imports). One file per part
(FILE-LAYOUT), matching the sibling `viz/` convention.

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

- **Tenancy / isolation (rule 6):** unchanged from the shipped dashboard plane — the workspace comes from
  the caller's token, never from imported JSON; a row cell and a variable definition are ordinary
  workspace-scoped bytes on the `dashboard:{id}` record. Every variable *resolver* is a `{tool,args}` MCP
  call that derives the workspace from the token (as today). The mapper's ws-isolation is owned by
  [`viz/import-export-scope.md`](viz/import-export-scope.md); the model changes here add no new isolation
  surface.
- **Capabilities (rule 5/7):** **no new host verb, no new cap** from this umbrella. Rows + variable fields
  ride the shipped `mcp:dashboard.save:call` (author) / `dashboard.get` (read); variable option resolution
  rides each resolver tool's own cap under `viz.query` / the resolver dispatch. Import/export caps stay in
  the mapper scope.
- **Placement (rule 1):** `either` — pure additive record fields + client-side row/variable rendering;
  one transport, no role branch.
- **MCP surface (§6.1):** the *conversion tool* is the existing `dashboard.import`/`dashboard.export`
  pair — bounded synchronous single-document ops in [`viz/import-export-scope.md`](viz/import-export-scope.md).
  This umbrella adds **no verbs**: rows and variables are additive fields on the record those verbs already
  read/write. A future **bulk / folder import** is that scope's named job follow-up, not here.
- **Data (SurrealDB):** rows and variable extensions are serde-default additive fields on the existing
  `dashboard:{id}` record (`cells[]` gains `view:"row"` cells; `variables[]` gains optional fields). No new
  table. State vs motion holds.
- **Bus (Zenoh):** none — record state, not motion.
- **Extensions (rule 10):** `view:"row"` and variable fields are opaque data; nothing branches on an
  extension id. A row can group `ext:<id>/<widget>` cells exactly like any other.
- **SDK/WIT impact:** the `Cell`/`Dashboard`/`Variable` shape gains additive fields (serde-default);
  the plugin boundary (an extension linking the frozen `vars` lib) is touched by the variables slice —
  **flagged loudly there**: a `VARS_LIB_V` bump if `VarScope`'s resolved shape changes.
- **Skill doc:** **N/A for this umbrella** — rows and variables add **no agent-/API-drivable surface** (no
  new MCP verb, no gateway route; they ride the shipped `dashboard.save`/`dashboard.get` + client
  resolution). The *conversion tool's* drivable surface — `dashboard.import`/`dashboard.export` — belongs to
  [`viz/import-export-scope.md`](viz/import-export-scope.md); if any surface warrants a
  `skills/<name>/SKILL.md`, that scope names it, not this one.

## Phasing

1. **Stage 1 — the audit (above). Done in this doc.** The gap matrix + the two traps + the triage. This is
   the "see what's missing" the user asked for; it is a decision record, not a placeholder.
2. **Stage 2 — panel rows** ([`panel-rows-scope.md`](panel-rows-scope.md)). Independent of variables;
   valuable to every native dashboard. Ship first (smaller, self-contained).
3. **Stage 3 — advanced variables** ([`dashboard-variables-advanced-scope.md`](dashboard-variables-advanced-scope.md)).
   The larger slice; touches the frozen `vars` lib.
4. **Stage 4 — the mapper consumes them.** [`viz/import-export-scope.md`](viz/import-export-scope.md)
   (already scoped) maps Grafana rows → our row cells and Grafana `VariableModel` → our extended `Variable`,
   and the audit's "degrade" list becomes its honest-placeholder set. Not re-scoped here.

## Testing plan

Per [`../../testing/testing-scope.md`](../../testing/testing-scope.md) — real gateway, real store, seeded
real rows, **no `*.fake.ts`** (a real Grafana export `.json` is a fixture, not a fake backend). The
cross-cutting gates the sub-scopes each satisfy:

- **Capability deny (mandatory):** saving a dashboard carrying row cells / extended variables without
  `mcp:dashboard.save:call` is denied opaquely (the shipped save gate; no new cap to test, but the deny path
  must stay green through the additive fields).
- **Workspace isolation (mandatory):** a ws-A dashboard with rows + chained variables is invisible to ws-B;
  a variable resolver call derives its workspace from the token (a ws-B viewer never resolves ws-A options).
- **Additivity:** a pre-rows / pre-advanced-variables record loads and round-trips **byte-clean** (the
  serde-default discipline — the headline regression guard for both slices).
- Slice-specific cases (row collapse membership, dual-encoding normalization; chained resolution order,
  label/value split, regex extraction) live in each sub-scope's testing plan.

## Risks & hard problems

- **Row dual-encoding** (collapsed-nested vs expanded-sibling) is the #1 mapper corruption source — the
  rows slice must pin one storage encoding and prove both Grafana encodings normalize to it.
- **Chained-variable ordering** has no explicit graph in Grafana JSON — we must derive it by parsing `$var`
  references, and a cycle must fail honestly, not hang.
- **The frozen `vars` lib** (`VARS_LIB_V`) is linked by extensions — a breaking `VarScope` change forces a
  major bump and a receiver rejection path. The variables slice must stay additive or bump loudly.
- **Scope creep into "Grafana the product".** The matrix's "out / degrade" columns are the guardrail; adhoc
  filters, annotations, links, snapshots are explicitly *not* this umbrella.

## Open questions

- **Row storage encoding:** store row membership **positionally** (child cells are the cells between this
  row's `y` and the next row's, matching Grafana's expanded model) or **explicitly** (a `rowId` on each
  child cell)? Lean positional (1:1 with Grafana expanded, no new per-cell field) — resolved in the rows
  scope. Answerable by which survives a drag-reorder without a membership rewrite.
- **Chained resolution transport:** resolve the dependency order **client-side** in the variable bar (each
  resolver is already a client-issued `{tool,args}` call) or push a "resolve all variables for this
  dashboard" batch to a host verb? Lean client-side (no new verb, matches today) — resolved in the
  variables scope; revisit only if per-variable round-trips are too chatty.
- **`datasource` variable type:** does it resolve against `datasource.list` (federation) directly, or is it
  a `source`-typed variable with a fixed resolver tool? Resolved in the variables scope.

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
