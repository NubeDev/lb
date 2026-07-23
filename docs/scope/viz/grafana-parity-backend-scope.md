# Viz scope — Grafana parity, the backend half (model fields, lb-viz coverage, the import pin)

Status: **scope (the ask)**. Backend-only — every UI concern (typed option shapes, editors,
renderers, the missing panel types) is owned by the downstream consumer's UI scope
(`rubix-ai: docs/scope/frontend/dashboard/viz/grafana-parity-ui-scope.md`). This doc is the
**upstream lb** half that scope's rows point at.

Audited against a fresh Grafana clone (`~/code/go/grafana`, **13.2.0-pre**, synced 2026-07-13).
Two pins the audit confirms: classic dashboard JSON is **still `schemaVersion: 42`** in 13.2, and
the v2 kind (`dashboard.grafana.app/v2beta1`, panels-in-`elements` + `layout` envelope) is what
new-layout dashboards export — we **target v1 and reject v2 with notice** (unchanged from the
conversion scope's call).

One paragraph: the shipped v3 panel model (`host/src/dashboard/model.rs`), the `lb-viz` crate, and
`viz.query` are already Grafana-shaped — verbatim transform ids, `targets[]` over a
`DataSourceRef`, opaque `fieldConfig`. This scope closes the **backend** gaps a real Grafana v1
import hits: a handful of additive serde fields on `Cell`/`Dashboard`/`Variable` (panel
`transparent`/time-override/links, dashboard `timezone`, variable `description`/`skipUrlSync`/
`allowCustomValue`), **tranche 2 of the lb-viz transformer set** (~6 high-frequency transforms +
the missing reduce calcs), and the **import-mapper backend contract** (`__inputs`/`__requires`
resolution, legacy string-datasource normalization, v2 detection) that `dashboard.import` and the
standalone converter both consume.

## Goals

- **Every additive model field a Grafana v1 page needs, named and serde-default.** No break to
  v1/v2/v3 cells; `fieldConfig` stays an opaque `Value` at this layer (the UI owns the typed shape).
- **lb-viz tranche 2:** the next transforms + reduce calcs ranked by real-dashboard frequency, one
  per file, Grafana-verbatim ids/options, pure.
- **One import pin, written down:** v1/42 accepted, `__inputs` resolved, v2 rejected-with-notice —
  the contract shared by `dashboard.import` (frontend/dashboard/viz/import-export-scope.md) and the
  standalone converter (frontend/dashboard/grafana-conversion-scope.md), defined once here.

## Non-goals

- **Nothing UI.** Typed per-view options, fieldConfig taxonomy (color modes, mappings), editors,
  renderers, new panel types — all in the rubix-ai UI scope. Rule of thumb: if `model.rs` stores it
  as opaque `Value`, it is not this scope's problem.
- **Not the v2 kind.** Detect and reject with a pointer; no `elements`/`layout` mapping.
- **Not annotations, dashboard links, snapshots, adhoc/groupby variables, panel/row repeat** — all
  stay degrade/out per the conversion scope's matrix; nothing here changes those calls.
- **Not a Grafana query-language shim.** A Grafana target's DS-specific body (`expr`, `rawSql`, …)
  maps to `tool`+`args` where a datasource mapping exists, else rides as opaque reported-degraded
  data. No Prometheus/SQL emulation.

## Stage 1 — the backend gap matrix (audited 2026-07-14, Grafana 13.2.0-pre)

### `Cell` (panel-level, `model.rs`) — Grafana `#Panel` fields we lack

| Grafana field | Fate | Note |
|---|---|---|
| `transparent: bool` | **Add** (serde-default) | Cheap, renderers honor it UI-side (render row claimed by the UI scope) |
| `timeFrom` / `timeShift` / `hideTimeOverride` | **Add** | Lands in a **new** typed `query_options` on `Cell` — see the P1 opener below; `viz.query` applies the override when dispatching targets |
| `interval` | **Map** | → `query_options.minInterval` (mapper rename) |
| panel-level `datasource` default | **Map** | Mapper folds into each target's `datasource` ref; no model field |
| panel `links: DashboardLink[]` | **Add** (opaque array) | UI renders; model carries |
| `id: number` | **Map** | → our string `i` (mapper stringifies; export re-numbers) |
| `gridPos.static` | **Drop-with-notice** | rgl `static` exists UI-side if ever wanted |
| `repeat` / `repeatDirection` / `maxPerRow` | **Degrade** | Unchanged (conversion-scope call) |
| `cacheTimeout` / `queryCachingTTL` | **Drop-with-notice** | Our cache is the dashboard-query-cache |

> **P1 opener — the `queryOptions` hole.** The editor-parity UI ships a cell field
> `queryOptions {maxDataPoints, minInterval, relativeTime}`, but `model.rs` has **no such field**
> and `Cell` has no serde catch-all — so `dashboard.save` very likely **silently drops it today**
> (unknown top-level cell fields are not opaque, unlike `options`/`fieldConfig`). P1 verifies
> this on the real save path, adds the typed `query_options` struct (the existing UI trio + the
> new `timeFrom`/`timeShift`/`hideTimeOverride`), and pins the round-trip test that would have
> caught it. This also bounds the UI scope's carry-don't-strip guarantee: it holds inside
> `options`/`fieldConfig`/`custom`, NOT for unknown top-level cell fields.

### `Dashboard` — dashboard-level fields we lack

| Grafana field | Fate | Note |
|---|---|---|
| `timezone` | **Add** (serde-default) | Render path resolves via user-prefs; record carries the import |
| `time` / `refresh` | **Map** | Live in routing/toolbar today; mapper seeds toolbar + initial range |
| `weekStart`, `fiscalYearStartMonth`, `graphTooltip`, `editable`, `liveNow`, `preload`, `timepicker` | **Degrade/drop-with-notice** | Unchanged |
| `annotations`, `links`, `snapshot` | **Out** | Unchanged |
| `uid`, `version`, `gnetId`, `revision` | **Map/drop** | `uid`→our id (re-slugged); rest dropped |

### `Variable` — fields/types we lack

| Grafana | Fate | Note |
|---|---|---|
| `description` | **Add** | |
| `skipUrlSync` | **Add** | Selection lives in URL today; this opts a var out |
| `allowCustomValue` | **Add** | multi/select UX flag, carried opaque until UI ships it |
| `hide`/`refresh`/`sort` as ints | **Map** | Enum-int → our string enums (mapper table). 13.x `VariableHide` is 0..3 — the new `3` (in-controls-menu) maps to our `dontHide` + report line; `sort` is the 0..8 enum |
| `VariableOption.text: string[]` | **Normalize** | Mapper joins/first; our option is `{text,value}` strings |
| types `adhoc`, `groupby`, `system`, `snapshot`, `switch` | **Degrade/out** | Unchanged |

### `lb-viz` — transforms (have 11 of Grafana's 43 `DataTransformerID`s) + reduce calcs (have 10 of ~124 `ReducerID`s)

Shipped: `reduce, organize, filterFieldsByName, filterByValue, groupBy, joinByField,
calculateField, sortBy, limit, merge, seriesToRows`.

**Tranche 2a (add — highest frequency in public dashboards):** `renameByRegex`, `filterByRefId`,
`convertFieldType`, `extractFields`, `labelsToFields`, `concatenate`.
**Tranche 2b (add if a fixture demands, else accept-and-report-degraded):** `groupingToMatrix`,
`rowsToFields`, `partitionByValues`, `prepareTimeSeries`, `formatTime`, `formatString`,
`joinByLabels`, `histogram`, `timeSeriesTable`, `heatmap`.
An unknown transform id in an imported panel is **carried opaque + reported**, never dropped —
`viz.query` already skips-with-notice rather than erroring.

**Reduce calcs to add** (`reducer.rs` ships 10 today, incl. `range` — `reducer.rs:31`; Grafana's
`fieldReducer` registers ~124 `ReducerID`s, each percentile individually `p1`…`p99`): `diff`,
`diffperc`, `delta`, `step`, `median`, `variance`, `stdDev`, `distinctCount`, `changeCount`,
`allIsZero`, `allIsNull`, plus **the general `pNN` pattern (1–99)** — the picker offers
`p25`/`p50`/`p75`/`p90`/`p95`/`p99`, but any imported `pNN` computes rather than degrades. Same
pure-`Option` contract (non-numeric/null cells skipped). Note the mirror is **already broken**:
the client `reduceCalc` has 9 calcs (no `range`) vs the backend's 10 — the UI scope names closing
that drift.

### The import pin (consumed by `dashboard.import` + the standalone converter)

- **Accept** classic v1 JSON, `schemaVersion <= 42`, porting the **migration subset already
  resolved in `import-export-scope.md` — that decision stands, this pin implements it** (do not
  re-litigate; the earlier draft of this pin that said "no migration chain, notice only" is
  superseded): the **v33** string-datasource-**name** → `{type, uid}` ref normalization
  (`DashboardMigrator.ts:551` — v36 only touches annotation datasources), the panel-type renames
  (`graph`→`timeseries`, `singlestat`→`stat`/`gauge`), and fieldConfig standardization. Anything
  older than the ported subset degrades with a version notice — never the full migration chain.
  (The standalone converter's *first cut* reads 42 as-is per its own non-goals and reports; on
  fold-in it inherits this pin.)
- **`__inputs` / `__requires` / `__elements`:** resolution is a **name-keyed lookup against the
  `__inputs` entries** — exactly Grafana's evaluator (`dash_template_evaluator.go`): no
  `DS_`/`VAR_` prefix magic, every `${NAME}` occurrence substituted from a caller-supplied input
  map, `pluginId == "__expr__"` inputs auto-fill, unresolved inputs an honest per-entry error.
  Grafana's backend strips only `__inputs`; **we strip all three** envelopes from the stored
  record (our deliberate delta, noted here), with `__requires` reported-informational and
  `__elements` library panels mapped to `panel:{id}` + `panelRef` cells first.
- **Reject** v2 (`elements`/`layout` shape or `apiVersion: dashboard.grafana.app/*`) with a
  pointer, and snapshots.
- Datasource `uid` remap (Grafana uid → our federation datasource) is the import verb's argument
  surface, per import-export-scope; special uids (`-- Mixed --`, `__expr__`, `${dsVar}`) degrade
  per-target with a report line.

## How it fits the core

- **Rule 10 / symmetric nodes:** every new field is generic model surface; the mapper treats panel
  types, datasources, and transform ids as opaque vocabulary — no special-casing a named extension
  or datasource. No role branch anywhere.
- **Tenancy (rule 6):** unchanged — import authority is the caller's token; a `${DS_*}` resolution
  can only name a datasource reachable in the caller's workspace.
- **Capabilities (rule 5/7):** no new verb in this scope. Model fields ride the existing
  `dashboard.save`; transforms ride `viz.query` (gated `mcp:viz.query:call`); the import pin is
  consumed by the already-scoped `dashboard.import`/`export` verbs when they land.
- **One datastore / additive discipline:** every field `#[serde(default)]`; `Cell.v`/`schemaVersion`
  untouched; Grafana JSON remains interchange, never storage.
- **FILE-LAYOUT:** one transform per file under `crates/viz/src/transforms/`; calcs stay in
  `reducer.rs` unless it breaches 400 lines (then `reducer/` folder-of-calcs).
- **Skill doc:** N/A — no new agent-drivable verb in this scope; `dashboard.import`'s drivable
  surface (and its skill doc decision) belongs to import-export-scope.

## Phasing

1. **P1 — model fields.** Opens with the `queryOptions` hole (verify the drop on the real
   `dashboard.save` path; add the typed `query_options` struct), then the remaining Add rows
   (`Cell`, `Dashboard`, `Variable`), serde-default, with round-trip tests. Release as a
   `node-v*` tag; rubix-ai bumps its pin.
2. **P2 — lb-viz tranche 2a + reduce calcs.** One transform per file + golden tests. Tranche 2b
   only as fixtures demand.
3. **P3 — the import pin as code.** The `__inputs` resolver + v1/v2 detector + the ported
   migration subset as a **small dep-light crate beside lb-viz** (e.g. `crates/grafana-map`,
   no host-crate dependency), so the standalone converter workspace consumes it as a plain git
   dep instead of vendoring — this resolves the conversion scope's vendor-vs-path question for
   *this module* (its `model.rs` mirroring is unchanged). The `dashboard.import` verb
   (import-export-scope) calls the same crate. Lands with whichever consumer builds first.
   Flag to the conversion scope: the caller-supplied input map means its one-screen UI grows an
   `__inputs` collection surface.

## Example flow

A user imports a schemaVersion-30 Grafana export: the pin module detects v1, applies the ported
subset — the string `"datasource": "Prometheus"` on each target becomes a `{type, uid}` ref (v33
rule), a `graph` panel becomes `view:"timeseries"`, `singlestat` becomes `stat` — then the
`__inputs` resolver substitutes the caller's uid for `${DS_PROMETHEUS}` (name-keyed lookup),
strips the three envelopes, and the mapper emits a `Dashboard` whose cells carry
`query_options.timeFrom` from the one panel that had a time override. `viz.query` later runs that
panel's `renameByRegex` transform (tranche 2a) and a `p90` reduce (the `pNN` pattern) — every
unmapped feature appears in the conversion report, nothing silently dropped.

## Testing plan

Per `scope/testing/testing-scope.md` — no fakes; real Grafana exports are **fixtures**.

- **Model round-trip:** each new field survives `serde_json` round-trip and defaults cleanly on a
  v1/v2/v3 cell without it (the additive guard). Headline case: a UI-shaped cell carrying
  `queryOptions` survives the real `dashboard.save` → `dashboard.get` path (the P1 opener's
  regression pin).
- **Transform goldens:** per new transform, fixture frames in → asserted frames out, options
  Grafana-verbatim; unknown-id carried + reported (the `viz.query` skip-with-notice test extends).
- **Calc table:** each new reducer against a shared numeric fixture incl. null/non-numeric skips.
- **Import-pin fixtures:** a real 13.2 export with `__inputs` (name-keyed resolution, all three
  envelopes stripped), a pre-v33 export with a string datasource **and** a `graph`/`singlestat`
  panel (normalized to a ref + renamed `timeseries`/`stat` — the ported subset, matching
  import-export-scope's pinned test), a v2beta1 export (rejected with pointer).
- Mandatory caps-deny / workspace-isolation gates: no new verb here, so the existing `viz.query`
  and `dashboard.save` gates are the surface — they stay green.

## Risks & hard problems

- **The `queryOptions` drop** (P1 opener) may mean shipped user data has already been lost on
  save — verify before fixing, and say so honestly in the session doc if confirmed.
- **Tranche creep.** Grafana's transformer set keeps growing (43 ids and counting); the bound is
  "2a + fixture-demanded 2b", everything else carried-opaque + reported. Revisit only with a
  fixture in hand.
- **`timeFrom`/`timeShift` semantics** interact with the dashboard time range inside `viz.query`
  (override vs shift math, `hideTimeOverride` is display-only). Pin Grafana's exact semantics in
  the P1 session doc before implementing.
- **Percentile calcs** need a sort — fine at panel row counts; note the same push-to-query bound
  as heavy aggregation (transformations-scope).

## Open questions

- Does `timezone` belong on the record or purely user-prefs? Lean record-carries-import,
  prefs-wins-at-render (matches the canonical-in/localized-out doctrine). Decide in P1.
- Exact crate name/placement for the P3 import module (`crates/grafana-map` vs a module inside a
  future import crate). Decide when P3 starts; the constraint is only "no host dependency".

## Related

- Downstream UI half: `rubix-ai` → `docs/scope/frontend/dashboard/viz/grafana-parity-ui-scope.md`
  (typed options, fieldConfig taxonomy, editors, renderers, the `text` panel revival).
- `frontend/dashboard/viz/import-export-scope.md` — the `dashboard.import`/`export` verbs that
  consume the import pin · `frontend/dashboard/grafana-conversion-scope.md` — the standalone
  converter (same pin) · `frontend/dashboard/viz/transformations-scope.md` — lb-viz tranche 1 +
  the `viz.query` doctrine this extends.
- [`grafana-dashboard-fidelity-scope.md`](grafana-dashboard-fidelity-scope.md) — the **measured**
  gap-closure campaign (lossless import + richer model + robust render), grounded in converting a real
  35-panel pdnsw board; consumes the additive fields (markdown view, y-clamp, repeat) named here.
- `rust/crates/host/src/dashboard/model.rs` (the v3 model) · `rust/crates/viz/` (transforms,
  reducer) · `rust/crates/host/src/viz/query.rs` (the verb).
- Grafana reference clone `~/code/go/grafana` (13.2.0-pre): `kinds/dashboard/dashboard_kind.cue` +
  `packages/grafana-schema/src/raw/dashboard/x/types.gen.ts` (v1, schemaVersion 42),
  `apps/dashboard/kinds/v2beta1/` (the rejected v2), `public/app/features/dashboard/state/DashboardMigrator.ts`,
  `pkg/services/dashboardimport/utils/dash_template_evaluator.go` (`__inputs`),
  `packages/grafana-data` `fieldReducer` (calcs), `packages/grafana-data/src/transformations`.
