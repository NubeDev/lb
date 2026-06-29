# Viz scope — the panel model (the Grafana-aligned cell, additive over v2)

Status: scope (the ask). The **spine** of the [`viz/`](README.md) slice — the data shapes every other
viz sub-scope references. Promotes to [`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md).

One paragraph: extend our durable **cell** record into a **Grafana-aligned panel model** — a `view`
(Grafana panel type), one or more **targets** (`sources[]`) over a **datasource ref**, a **`fieldConfig`**
(field option defaults + per-field overrides), a **`transformations[]`** pipeline, structured per-view
**`options`**, and a 24-column grid — all **serde-default additive** over the shipped v2 cell, with a
dashboard-level **`schemaVersion`** so import/export and migration have a version to pin. We store our own
record (one datastore, the shipped contract intact); Grafana JSON is an interchange format mapped at the
edge ([`import-export-scope.md`](import-export-scope.md)).

## Goals

- **One additive shape** that is a superset of the shipped `Cell` and maps 1:1 onto a Grafana panel. No
  break to v1 (series binding) or v2 (`view`+`source`) cells; every new field is `#[serde(default)]` /
  `field?` and absent on an old cell.
- **`view` adopts Grafana's panel-type ids** (`timeseries`, `barchart`, `stat`, `gauge`, `bargauge`,
  `table`, `piechart`, `histogram`, `state-timeline`, `text`, … plus our `ext:<id>/<widget>`). The shipped
  `chart`/`stat`/`gauge`/`table`/`plot`/`d3`/`template`/control views remain valid aliases (a v2 `chart`
  cell *is* a `timeseries` panel). [`chart-types-scope.md`](chart-types-scope.md) owns the set.
- **Targets = `sources[]`.** Generalize the single `source { tool, args }` to an ordered `sources[]`
  (Grafana `targets[]`), each carrying a `refId` (A, B, C…) and a `datasource` ref. One source stays the
  common case; a v2 single-`source` cell reads as a one-element `sources[[A]]`.
- **`fieldConfig` and `transformations[]` as first-class additive fields**, defined here in shape and
  owned in depth by [`field-config-scope.md`](field-config-scope.md) and
  [`transformations-scope.md`](transformations-scope.md).
- **A `schemaVersion` on the dashboard** so import/export/migration pin a version, mirroring Grafana.

## Non-goals

- **No Grafana-native persistence.** We don't store Grafana JSON; we store our record and map at the edge.
- **No new datastore / no new dashboard verb for the shape.** The extended cell rides the existing
  `dashboard.save`/`get` UPSERT — it's the same record with more (defaulted) fields. (Import/export add
  their *own* verbs — that's [`import-export-scope.md`](import-export-scope.md), not this shape change.)
- **No options semantics here.** This doc fixes the *shape*; what each field *means* and renders is the
  chart-types / field-config / transformations docs.

## The shape (additive over the shipped `Cell`)

The shipped `Cell` (see `ui/src/lib/dashboard/dashboard.types.ts` + the host `Dashboard` record) gains:

```
Cell {                                   // existing
  i, x, y, w, h            // grid geometry — pin the grid to a 24-col width (Grafana's gridPos.w ∈ 1..24)
  v?: number               // contract version (absent/1 = v1, 2 = v2, 3 = this panel model)
  widget_type              // v1 fallback (chart|stat|gauge)
  title?                   // header label
  view?: View              // v2/v3 render vocabulary → Grafana panel `type` (timeseries|barchart|…)
  source?: Source          // v2 single source (kept; reads as sources[0])
  action?: Action          // v2 control write
  options?: Record<…>      // per-view options (now structured per `view`)

  // --- added by this scope (all serde-default / optional) ---
  description?: string                   // panel description (Grafana parity)
  sources?: Target[]       // v3 targets — supersedes single `source`; sources[0] === source for v2 compat
  transformations?: Transformation[]     // the client-side pipeline (transformations-scope)
  fieldConfig?: FieldConfig              // defaults + per-field overrides (field-config-scope)
  pluginVersion?: string                 // for import/export round-trip fidelity
}

Target {                                 // a Grafana "target" = one query against one datasource
  refId: string            // "A" | "B" | … — referenced by transformations + overrides
  datasource?: DataSourceRef             // which datasource (native|series|federation) — datasource-binding-scope
  tool: string             // the resolved MCP tool (store.query | series.read | federation.query | ext tool)
  args?: Record<string, unknown>         // the query args (sql, series, source+sql for federation, …)
  hide?: boolean           // skip this target's data (Grafana parity)
}

DataSourceRef { type: string; uid?: string }   // type: "surreal" | "series" | "federation" | ext id; uid: a datasource:{ws}:{name}

FieldConfig { defaults: FieldOptions; overrides?: FieldOverride[] }   // field-config-scope owns FieldOptions/FieldOverride
Transformation { id: string; options?: Record<string, unknown>; disabled?: boolean; filter?: Matcher }  // transformations-scope
```

And the dashboard record gains:

```
Dashboard {                              // existing: id, title, owner, visibility, cells[], variables?, updated_ts, deleted?
  schemaVersion?: number                 // OUR panel-model version (not Grafana's); pinned at save, migrated on load
  // time range / refresh already live in the URL (routing-scope) — not duplicated on the record
}
```

### The `v`/`schemaVersion` discipline (two versions, distinct on purpose)

- **`Cell.v`** is the **contract** version (the v1→v2 bridge contract; this scope is **v3**, additive). A
  renderer reads the highest shape it understands and falls back: `view` → `widget_type`; `sources` →
  `source` → `binding`. An unknown major `v` is rejected by a receiver (the shipped rule).
- **`Dashboard.schemaVersion`** is **our** dashboard-document version, used by the **import/export +
  migration** path to know how to read an older saved dashboard. It is *not* Grafana's `schemaVersion`
  (that one lives only in the Grafana JSON and is consumed by the mapper —
  [`import-export-scope.md`](import-export-scope.md)).

The two never conflict: `v` versions the cell *contract* (what a bridge accepts); `schemaVersion` versions
the stored *document shape* (what a migration reads). Both are additive and defaulted.

## How it fits the core

- **Tenancy / isolation:** unchanged — the extended cell is part of the workspace-scoped `dashboard:{id}`
  record; a `Target.datasource` ref resolves only within the caller's workspace
  ([`datasource-binding-scope.md`](datasource-binding-scope.md)). No new key, no new wall.
- **Capabilities:** the shape change touches **no** capability — it rides `dashboard.save`/`get`
  (`mcp:dashboard.save:call` / `:get`), already gated. A target's data is still leashed by the target
  tool's cap ∩ grant, host-re-checked per call. New caps belong to import/export, not here.
- **Placement:** `either` — pure record shape; same on edge and cloud.
- **MCP surface (§6.1):** **none added.** The extended cell is part of the layout UPSERT (one synchronous,
  bounded write). Multiple targets do **not** mean a batch — each target is a normal `bridge.call` at
  render time, leashed as today; an unbounded target must itself be a job-backed tool (the shipped rule).
- **Data (SurrealDB):** one record, more defaulted fields. Bounded: `fieldConfig.overrides[]` and
  `transformations[]` are capped (e.g. ≤64 overrides / ≤32 transforms per panel) to keep the dashboard
  record small; inline scripted code stays the shipped ≤4 KB / `render_template:{id}` rule.
- **Bus (Zenoh):** unchanged — live targets stream over the shipped series/bus SSE; state vs motion holds.
- **Sync / authority:** the record is the shipped `(table,id)` UPSERT on the §6.8 sync path; additive
  fields replay idempotently. An older node that doesn't understand `sources[]`/`fieldConfig` reads the
  cell via the `source`/`widget_type` fallback (forward-compatible by serde-default).
- **Secrets:** none reach the cell; a federation target's DSN stays server-side (datasources doctrine).
- **SDK/WIT impact:** the **cell contract goes v2 → v3** (additive: `sources[]`, `fieldConfig`,
  `transformations`, structured `options`). Flag it — it is additive (a v2 cell is a v3 cell with one
  target and an empty field-config), with a `v` bump so a future v4 is additive too. The `[[widget]].scope`
  / bridge contract is **unchanged**.

## Example flow

1. A user opens the panel editor on an empty cell and picks **Timeseries**. The cell is `{ v:3,
   view:"timeseries", sources:[{refId:"A", datasource:{type:"surreal"}, tool:"store.query", args:{sql}}],
   fieldConfig:{ defaults:{ unit:"celsius", decimals:1 } }, options:{ legend:{showLegend:true} } }`.
2. The editor adds a second target **B** against a federation datasource (`datasource:{type:"federation",
   uid:"datasource:kfc:timescale"}`, `tool:"federation.query"`); both targets render as series.
3. A `reduce` transformation collapses each series to its last value for a companion `stat` panel — the
   `transformations[]` array, applied client-side over the targets' rows.
4. The user sets a threshold (`fieldConfig.defaults.thresholds`) coloring the line red over 5 °C.
5. `dashboard.save` UPSERTs the cell — same verb, same record, more defaulted fields. A reload re-reads it;
   an older client reads it via `source`/`view` fallback (no crash).

## Testing plan

Per [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — real gateway/store, no fakes.

- **Backward compatibility (the headline):** a seeded **v1** series cell and a **v2** `chart`+`store.query`
  cell both load, render, and re-save through the v3 shape **unchanged** (round-trip identity for the
  fields they set; new fields stay absent/defaulted).
- **Additive serde:** a v3 cell with `sources[]`/`fieldConfig`/`transformations` round-trips through
  `dashboard.save`/`get`; a client ignoring the new fields still renders via `source`/`view`.
- **Bounds:** a panel exceeding the override/transform cap is rejected (or truncated honestly), not
  silently stored unbounded.
- **Workspace isolation:** unchanged-but-asserted — a v3 cell's `Target.datasource` ref in ws-B resolves
  only ws-B (covered fully in [`datasource-binding-scope.md`](datasource-binding-scope.md)).
- **Capability deny:** save/get still gated; the shape change adds no bypass.

## Risks & hard problems

- **Two version fields is a footgun if conflated.** `Cell.v` (contract) vs `Dashboard.schemaVersion`
  (document) must stay distinct in code and docs; a migration that bumps the wrong one corrupts reads. Name
  them unmistakably.
- **`source` → `sources[]` migration.** The read path must treat a single `source` as `sources[0]`
  everywhere (render, vars interpolation, the bridge leash set) or a v2 cell loses its data. One adapter
  function, tested both directions.
- **Record growth.** `fieldConfig.overrides[]` + multi-target + transforms can bloat the dashboard record;
  the caps above are load-bearing for roster/list performance.
- **Grid width.** Grafana is 24-col; if our grid is a different width, import scales `gridPos` lossily.
  Pin our grid to 24 (a small change to `Grid.tsx`) so import is exact.

## Resolved decisions

No blocking open questions — these are the long-term answers the build follows.

- **Pin the grid to 24 columns now.** Grafana's `gridPos.w ∈ 1..24`; pinning our grid to 24 makes import
  exact and is a one-line `Grid.tsx` config. Responsiveness stays a `ui-standards` concern (breakpoint
  re-stacking), not a column-count change.
- **Define `sources[]` in the shape from day one**, so the contract is stable, even though the Phase-1
  editor may expose a single target. A v2 single-`source` cell reads as `sources[0]` through one adapter
  function; multi-target authoring lands with [`datasource-binding-scope.md`](datasource-binding-scope.md).
- **Keep our string `i` as the cell key; do not add Grafana's numeric `panel.id` to the record.** The
  import/export mapper carries a numeric `id` internally for round-trip fidelity only
  ([`import-export-scope.md`](import-export-scope.md)) — it never lands on our stored cell.

## Related

- [`README.md`](README.md) — the viz umbrella + the reconciliation table.
- [`chart-types-scope.md`](chart-types-scope.md) (the `view` set) · [`field-config-scope.md`](field-config-scope.md)
  (`FieldConfig`/`FieldOptions`) · [`transformations-scope.md`](transformations-scope.md) (`Transformation`)
  · [`datasource-binding-scope.md`](datasource-binding-scope.md) (`DataSourceRef`/`Target`) ·
  [`import-export-scope.md`](import-export-scope.md) (`schemaVersion` + the mapper).
- [`../widget-builder-scope.md`](../widget-builder-scope.md) — the v2 cell this extends to v3.
- `ui/src/lib/dashboard/dashboard.types.ts` — the shipped `Cell`/`Dashboard` types to extend.
- `/tmp/grafana/kinds/dashboard/dashboard_kind.cue` — the Grafana panel/dashboard shape we align to.
- README **§3** (rules 1/2/3/6), **§6.1** (API shape).
