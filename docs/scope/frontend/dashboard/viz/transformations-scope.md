# Viz scope — the transformation pipeline (Grafana's transforms, client-side, bounded)

Status: scope (the ask). Part of the [`viz/`](README.md) slice — owns the `Transformation` shape the
[spine](panel-model-scope.md) declares. Promotes to [`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md).

One paragraph: add an **ordered, client-side transformation pipeline** to the panel model — Grafana's
`transformations[]`, adopted **verbatim in config shape and id set** so an imported dashboard maps 1:1.
Each transform is a pure `(frames, options) => frames` step; the pipeline runs over the **merged rows the
targets (`sources[]`) already returned**, *after* fetch and *before* `fieldConfig` formatting + rendering.
It is **post-fetch shaping, not a new query** (state vs motion): no new compute plane, no new datastore, no
new capability, no MCP surface — a pure TS lib (like the shipped vars lib) that the shell or a federated
remote can both run. The heavy lifting (aggregation over large data) stays **pushed down** into the query
(SurrealQL `GROUP BY`, `federation.query` SQL) or a job-backed tool; a transform only re-shapes the
**already-bounded** result the source tool capped. This doc owns the `Transformation` shape and the runner;
[`field-config-scope.md`](field-config-scope.md) owns the `Matcher` a transform's `filter` reuses.

## Goals

- **Adopt Grafana's transformation model verbatim.** `Transformation { id, options?, disabled?, filter? }`
  maps 1:1 onto Grafana's `DataTransformerConfig { id, options, disabled, filter, topic }` (we drop/defer
  `topic` until annotations exist). The id set is Grafana's id set (`reduce`, `organize`,
  `filterFieldsByName`, `filterByValue`, `sortBy`, `limit`, `groupBy`, `merge`, `joinByField`,
  `calculateField`, …) so import is a pass-through, not a translation.
- **An ordered pipeline.** `transformations[]` is applied in array order; each step takes frames and
  returns frames; `disabled` skips a step (kept in config for round-trip); `filter` (a `Matcher`) scopes a
  step to a subset of frames/fields.
- **Pure, one-responsibility-per-file.** `ui/src/lib/transforms/<id>.ts`, each a pure
  `(frames, options, ctx) => frames`, plus a `registry.ts` and a `runPipeline(frames, transformations[])`.
  No DOM, no fetch, no state — reusable by a federated extension exactly like the shipped vars lib.
- **Bounded by construction.** Transforms operate only on the bounded result the source tool already
  capped (`store.query` 10k/5s, `series.read` bounded — §6.1); a transform must never loop unboundedly in
  a render. Heavy aggregation is **pushed to the query or a job**, never an unbounded client transform.
- **Phased id coverage**, with **honest degradation**: an unsupported transform on import is
  **preserved-but-disabled with a visible note**, never silently dropped or faked.

## Non-goals

- **No server-side transform engine.** We do not fork compute into a new "transform" plane (rejected
  below). Transforms are client TS over already-fetched rows.
- **No new query.** A transform never issues a `bridge.call`; it re-shapes data already returned. Want more
  rows / a real aggregate? Change the **target's query**, not the transform.
- **No persisted transform output.** Only the `transformations[]` **config** persists on the cell (part of
  `dashboard.save`); the computed frames are ephemeral render state, recomputed each load.
- **No new datastore, no new capability, no MCP surface.** Transforms touch already-workspace-scoped rows.
- **No formatting here.** Units/decimals/dates are `fieldConfig` + the user-prefs bridge, *after* the
  pipeline ([`field-config-scope.md`](field-config-scope.md)).

## Intent / approach

A Grafana dashboard's `transformations[]` is an ordered list of pure data re-shaping steps. We adopt its
**config shape and id taxonomy unchanged**, and run the pipeline **client-side, post-fetch**:

```
sources[] → bridge.call each target → rows → adapter → Frame[]
          → runPipeline(frames, transformations[])      ← THIS DOC
          → fieldConfig formatting (units via user-prefs)
          → renderer (recharts/visx)
```

**Where it runs is the load-bearing decision.** Transforms run **client-side over the bounded rows the
target already returned** — pure TS in the shell (Tauri/browser) or a federated remote alike. This keeps
the slice **symmetric** (rule 1: no role branch, no server step) and adds **no new compute plane** (rule 2:
one datastore, no second engine). It honors **state vs motion** (rule 3): the query/bus already moved the
data; a transform only shapes the snapshot.

**The bound (stated explicitly).** A transform operates on the bounded result the source tool **already
capped**. The runner enforces a frame-size guard (e.g. refuse / truncate-with-note over a per-panel row
budget) so a transform can never become an unbounded render loop. **Heavy aggregation belongs in the
query** — SurrealQL `GROUP BY`/`count()`, `federation.query` SQL aggregates — or in a **job-backed tool**
([`../../../jobs/jobs-scope.md`](../../../jobs/jobs-scope.md), §6.10), not a client transform over a
deliberately-huge result. `groupBy`/`reduce` client-side are for the *last-mile* re-shape of an
already-small result (collapse a few series to one stat), not for crunching raw rows.

**Rejected alternative — a server-side transform engine.** Run transforms in the host (a `transform.run`
tool, or inside the bridge). Rejected: it forks compute into a new plane the platform doesn't have, breaks
"one datastore / no new engine" (rule 2), re-implements what the **query already does better** (push-down),
and is *not symmetric* (it would be a server-only code path). The data is already fetched and bounded;
shaping it is presentation, and presentation runs where the panel renders. The only "server-side" answer to
"too much data to transform on the client" is **change the query or use a job** — both shipped planes.

## How it fits the core

- **Tenancy / isolation:** unchanged. Transforms run on rows a target **already** fetched, which were
  workspace-scoped at the source (the bridge derived the workspace from the token). No new key, no wall
  crossing — the data is already inside the caller's workspace.
- **Capabilities:** **none added.** No new tool call, no new cap — the rows were leashed by the target
  tool's cap ∩ grant when fetched. A transform cannot reach data the target couldn't.
- **Placement:** `either` — pure TS, identical on edge and cloud, in the shell or a federated remote.
- **MCP surface (§6.1):** **none.** It's a pure client lib; `transformations[]` rides the existing
  `dashboard.save`/`get` UPSERT as config. A transform issues zero `bridge.call`s.
- **Data (SurrealDB):** only the **config** persists — `transformations[]` on the cell, bounded **≤32
  transforms/panel** (the panel-model cap). Computed frames are never stored. `options` payloads stay
  small (the shipped inline-size discipline); a transform that needs a big script is the wrong tool.
- **Bus (Zenoh):** unchanged. A live target streams over the shipped SSE; the pipeline re-runs over each
  bounded window the stream delivers (recompute on new data), never holding unbounded history.
- **Sync / authority:** `transformations[]` is part of the dashboard `(table,id)` UPSERT on the §6.8 sync
  path; it is serde-default additive and replays idempotently. An older node ignores it and renders the
  untransformed target rows (forward-compatible).
- **Secrets:** none reach a transform; it sees only already-fetched, already-redacted rows.
- **SDK/WIT impact:** none to the host contract. The cell's `transformations[]` is part of the v3 panel
  model (panel-model-scope); a v2 cell is a v3 cell with an empty pipeline.

### The data shape transforms operate on

Transforms are **field-oriented** (Grafana works in columnar `DataFrame`s), so the pipeline operates on a
small **columnar `Frame`**, not raw row objects:

```
Frame  { refId?: string; name?: string; fields: Field[]; length: number }
Field  { name: string; type: "number"|"string"|"time"|"boolean"|"other"; values: unknown[]; config?: FieldConfigDefaults }
```

`useSource` already normalizes a tool result to **rows** today; we add **one adapter** `rowsToFrame(rows)`
(infer field names/types from the first rows) at the pipeline's mouth and a `frameToRows(frame)` at its tail
for renderers that still want rows. We **lean columnar** because every Grafana transform is field-oriented
(reduce/organize/calculateField/joinByField all address *fields*) — matching Grafana means the transforms
port with near-zero impedance, and the adapter is a single, tested seam against the shipped row shape. (The
rejected alternative — keep everything as rows and re-derive columns per transform — re-implements Grafana's
field model badly and N times.) `Transformation { id, options?, disabled?, filter?: Matcher }`; `filter`'s
`Matcher` is the **same** shape `fieldConfig.overrides[]` uses, defined in
[`field-config-scope.md`](field-config-scope.md) (one matcher model across the slice).

### Phasing the transformer set

- **Phase 1 (high-value, simple):** `reduce`, `organize` (rename/reorder/hide fields),
  `filterFieldsByName`, `filterByValue`, `sortBy`, `limit`, `groupBy`, `merge`/`joinByField`,
  `calculateField`. This covers the common "stat from a series", "rename + hide columns", "join two
  targets", "computed field" needs that the panel editor's Transform tab exposes first.
- **Phase 2:** `renameByRegex`, `labelsToFields`, `configFromData`, `convertFieldType`,
  `formatTime`/`formatString`, `histogram`. (`configFromData`/`convertFieldType` interact with
  `fieldConfig` — sequenced after Phase 1 so the field model is settled.)
- **Deferred (named follow-ups, not silent gaps):** `spatial`, `partitionByValues`, `groupingToMatrix`,
  `regression`, `heatmap`, `transpose`, `joinByLabels`, `rowsToFields`, `extractFields`, `fieldLookup`,
  `seriesToRows`, `concatenate`, `ensureColumns`, `filterFrames`, `timeSeriesTable`, `noop`. An unsupported
  id encountered on import (or in a saved dashboard from a newer client) is **preserved in config, marked
  `disabled` with a visible "unsupported transform — preserved, not applied" note** in the editor — never
  dropped, never faked. This keeps round-trip fidelity and tells the user the truth.

### Interaction with `fieldConfig` and variables

- **Order:** transforms run **first**, then `fieldConfig` formats the resulting fields (units/decimals via
  the user-prefs `format.*` bridge), then the renderer draws. A transform shapes *data*; fieldConfig shapes
  *presentation* — never the reverse.
- **Variables:** a transform's `options` may reference dashboard variables (e.g. a `filterByValue`
  threshold of `$min`); the runner **interpolates** option strings via the **shipped vars lib** before the
  step runs, so transforms respond to the dashboard's variable state like queries do.

## Example flow

1. A panel has two targets — `A` (`store.query` over `readings`) and `B` (`federation.query` over a
   Timescale source) — each returning bounded rows. The bridge fetches both; the adapter yields two
   `Frame`s (`refId:"A"`, `refId:"B"`).
2. `runPipeline` applies `transformations[]` in order: `joinByField { byField:"ts" }` merges A and B on
   timestamp into one frame.
3. `calculateField { mode:"binary", left:"A.temp", op:"-", right:"B.temp", as:"delta" }` adds a computed
   `delta` field — pure arithmetic over existing values.
4. `filterByValue { delta > $threshold }` drops rows under the variable-interpolated threshold;
   `sortBy { field:"delta", desc:true }` then `limit { 50 }` bound the output (already bounded; this is
   last-mile shaping).
5. `fieldConfig.defaults.unit:"celsius"` formats `delta` via the user-prefs bridge; the `table` renderer
   draws it. Nothing was queried again; nothing was stored except the `transformations[]` config on save.
6. On import of a Grafana dashboard using `spatial` (deferred), the transform is preserved in config,
   shown disabled with the "unsupported — preserved, not applied" note; every other transform applies.

## Testing plan

Per [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — real gateway/store, seeded
real rows, **no `*.fake.ts`**. The pipeline is pure TS, so most cases are unit tests over seeded frames;
the end-to-end cases drive a panel against a real spawned gateway.

- **Per-transform purity (Phase 1, each id):** seed a known `Frame`, apply the transform, assert the exact
  output frame. Determinism + no mutation of input (frozen-input test).
- **Ordered pipeline:** a multi-step `transformations[]` (join → calculateField → filter → sort → limit)
  over seeded A/B frames produces the expected final frame; reordering steps changes the result as Grafana
  would.
- **`disabled` + `filter`:** a `disabled` step is skipped but round-trips in config; a `filter` (Matcher)
  scopes a step to the matching fields/frames only (shares the field-config matcher tests).
- **Variable interpolation:** a `filterByValue` referencing `$threshold` uses the resolved variable value
  (drive the shipped vars lib, not a stub).
- **The bound (regression):** a transform over an at-cap source result stays within the frame-size guard;
  a deliberately oversized frame is truncated-with-note or refused — **never an unbounded loop** (assert it
  returns within budget). This is the headline "no new compute plane" guard.
- **Order vs fieldConfig:** assert transforms run before formatting (a `convertFieldType` to number then a
  `unit` format renders correctly; the reverse order would not).
- **Honest degradation:** a seeded dashboard with an unsupported transform id loads, marks it
  disabled-with-note, applies the rest, and **re-saves the unsupported id unchanged** (round-trip).
- **End-to-end (real gateway):** a seeded `readings` table + a panel with a Phase-1 pipeline renders the
  transformed result through `dashboard.get` → `useSource` → pipeline → render.
- **Capability deny / workspace isolation (mandatory):** unchanged-but-asserted — transforms add no tool
  call, so a denied target yields no rows to transform; a ws-B panel's transforms only ever see ws-B rows
  (the wall is at fetch, the transform is downstream of it).

## Risks & hard problems

- **The bound is the whole game.** A client transform over an unbounded result is the failure mode this
  doc exists to prevent. The frame-size guard + "push aggregation to the query/job" rule must be enforced
  in the runner and stated in the editor, or a user writes a transform that hangs the render.
- **`calculateField` is a mini expression surface.** Keep it to Grafana's bounded modes (binary op,
  reduce-row, index, unary) — no arbitrary eval. An expression that needs more is a query or a job.
- **Frame model drift.** One `rowsToFrame`/`frameToRows` adapter, tested both directions, or the columnar
  model and the shipped row shape diverge. The adapter is the single seam; do not let transforms touch
  rows directly.
- **Import fidelity of options.** Grafana transform `options` shapes are deep and per-id; the Phase-1 ids
  must match Grafana's option keys exactly (cloned reference) or imported transforms misbehave silently.
- **Recompute cost on live streams.** Re-running the pipeline on every SSE window can be hot; memoize on
  `(frames, transformations[])` identity and keep Phase-1 transforms O(n) over the bounded window.

## Resolved decisions

No blocking open questions — these are the long-term answers the build follows.

- **The canonical pipeline type is a columnar `Frame`** (matches Grafana so transforms port cleanly), with
  one `rowsToFrame`/`frameToRows` adapter at both ends. Transforms never touch rows directly.
- **Enforce the frame-size bound in the runner, once**, with a per-panel row budget; individual transforms
  stay pure and unaware. The few inherently-expanding transforms (`histogram`, future `groupingToMatrix`)
  also self-check.
- **Defer `topic` (series/annotations/alertStates).** No annotations plane exists yet; keep the field in
  config for round-trip fidelity and ignore it on apply (honest degradation).
- **No separate transform version field.** Transforms ride the cell's `v` / the dashboard `schemaVersion`;
  a transform-lib version is a code concern, surfaced only if a future import needs it.

## Related

- [`README.md`](README.md) — the viz umbrella (the reconciliation table; Phase 3 = transforms).
- [`panel-model-scope.md`](panel-model-scope.md) — the spine; declares `Transformation` on the cell and the
  ≤32/panel bound this doc owns in depth.
- [`field-config-scope.md`](field-config-scope.md) — owns the `Matcher` a transform's `filter` reuses, and
  the formatting that runs **after** the pipeline.
- [`chart-types-scope.md`](chart-types-scope.md) — the renderers the transformed frames feed.
- [`datasource-binding-scope.md`](datasource-binding-scope.md) — `sources[]`/targets whose rows the
  pipeline merges.
- [`import-export-scope.md`](import-export-scope.md) — maps Grafana `transformations[]` 1:1 and drives the
  preserve-but-disable degradation for unsupported ids.
- [`panel-editor-scope.md`](panel-editor-scope.md) — the Transform tab that edits this pipeline.
- [`../widget-builder-scope.md`](../widget-builder-scope.md) — the v2 cell/`useSource` row shape the adapter
  bridges from.
- [`../../../jobs/jobs-scope.md`](../../../jobs/jobs-scope.md) — where heavy aggregation goes instead of an
  unbounded client transform.
- [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — the real-gateway/no-fakes test
  doctrine.
- The Grafana reference clone at `/tmp/grafana` —
  `packages/grafana-data/src/transformations/transformers/ids.ts` (the id set + `DataTransformerConfig`).
- README **§6.1** (API shape/bounds), **§6.10** (jobs/batch), **§3** (rules 1/2/3 — symmetric, one
  datastore, state vs motion).
