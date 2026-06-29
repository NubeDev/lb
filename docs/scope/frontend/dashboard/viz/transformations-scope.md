# Viz scope — the transformation pipeline (Grafana's transforms, backend-resolved, canonical frames)

Status: **SHIPPED (2026-06-29)** — Phase 3 of the [`viz/`](README.md) slice. `lb-viz` (the pure transform
lib, one transformer per file) + the `viz.query(panel) -> {frames, rows}` host verb (gated
`mcp:viz.query:call`, dispatching each target under `caller ∩ grant` by re-entering the host dispatcher,
running the pipeline server-side) shipped end to end. Promoted to
[`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md); session
[`dashboard-viz-phase3`](../../../../sessions/frontend/dashboard-viz-phase3-session.md). `viz.stream` (live
frames) remains the named follow-up. Original ask below.

Part of the [`viz/`](README.md) slice — owns the `Transformation` shape the
[spine](panel-model-scope.md) declares, **and** the backend panel-data resolver (`viz.query` + the `lb-viz`
lib). Promotes to [`public/frontend/dashboard.md`](../../../../public/frontend/dashboard.md).

One paragraph: a panel's data is **resolved in the backend** — a host verb `viz.query(panel) -> { frames }`
runs the panel's targets (`sources[]`, each an already-gated `store.query`/`series.*`/`federation.query`
call under `caller ∩ grant`, workspace from the token) and then applies the **transformation pipeline** (a
pure Rust lib, `lb-viz`) over the merged rows, returning **canonical** columnar frames. Grafana's
`transformations[]` config + id set are adopted **verbatim** so an imported dashboard maps 1:1. The decisive
reason it's backend, not client: **every client must get identical data without re-implementing the
pipeline** — the React web shell, a **React Native app**, a server-rendered email, a webhook all call one
verb and render the same frames. This is the same doctrine that puts formatting behind `format.*`
([`user-prefs-scope.md`](../../../prefs/user-prefs-scope.md)): one correct implementation, thin clients
free. `viz.query` returns **canonical** values only — presentation stays the separate `format.*` boundary
([`field-config-scope.md`](field-config-scope.md)); data shape is backend, formatting is a distinct call.

## Goals

- **A backend resolver verb, `viz.query`.** Input a panel spec (`{ targets[], transformations[] }`, or a
  saved panel ref); output `{ frames }` — canonical, bounded columnar frames ready to render. The host
  dispatches each target under the caller's authority, then runs the pipeline. One call, every client
  identical.
- **A pure Rust transform lib, `lb-viz`.** `transform(frames, &[Transformation]) -> frames`, one
  responsibility per file (`lb-viz/src/<id>.rs`), no I/O, no store, no bus — compiled into every node
  (symmetric, rule 1), the structural twin of `lb-prefs` (`uom`/`icu4x`). It is the *one* implementation of
  reduce/organize/filter/groupBy/joinByField/calculateField/… for the whole platform.
- **Adopt Grafana's transformation model verbatim.** `Transformation { id, options?, disabled?, filter? }`
  maps 1:1 onto Grafana's `DataTransformerConfig { id, options, disabled, filter, topic }` (we keep `topic`
  in config for round-trip, defer it on apply). The id set is Grafana's id set, so import is a pass-through,
  not a translation.
- **An ordered pipeline, bounded by construction.** Applied in array order; `disabled` skips a step (kept
  for round-trip); `filter` (a `Matcher`) scopes a step. Each target's result is already capped
  (`store.query` 10k/5s, `series.read` bounded, `federation.query` row-capped); the resolver enforces a
  per-panel frame budget so the pipeline can never run unbounded. **Heavy aggregation is pushed down into
  the query** (SurrealQL `GROUP BY`, `federation.query` SQL) or a job — `lb-viz` does the last-mile,
  cross-target shaping SQL can't.
- **Phased id coverage with honest degradation.** An unsupported transform is **preserved-but-disabled with
  a visible note**, never silently dropped or faked.

## Non-goals

- **No client-side transform pipeline.** We do **not** ship a TS reduce/join/calculate lib in the shell
  (rejected below — it forces every other client to re-implement it). The client renders the frames
  `viz.query` returns.
- **No formatting in `viz.query`.** It returns **canonical** values (UTC instants, SI/base units,
  locale-neutral); units/decimals/dates are applied afterward via the `format.*` prefs boundary
  ([`field-config-scope.md`](field-config-scope.md)). Data shape and presentation are distinct verbs.
- **No new query semantics / no new datastore.** `viz.query` issues no SQL of its own; it *dispatches the
  panel's existing target tools* and shapes their bounded results. SurrealDB stays the authority.
- **No persisted transform output.** Only the `transformations[]` **config** persists on the cell (part of
  `dashboard.save`); the resolved frames are ephemeral, recomputed per call.
- **No `calculateField` arbitrary eval.** Only Grafana's bounded modes (binary op, reduce-row, index,
  unary). An expression that needs more is a query or a job.

## Intent / approach

```
client (web | React Native | email | webhook)
   └─ viz.query(panel)  ──────────────────────────────►  HOST
                                                          ├─ for each target ∈ sources[]:
                                                          │    dispatch tool (store.query | series.* |
                                                          │    federation.query) under caller ∩ grant,
                                                          │    workspace from token, bounded → rows → Frame
                                                          ├─ lb-viz: transform(frames, transformations[])
                                                          └─ return { frames }  (CANONICAL, bounded)
   ◄─ frames ─ render (recharts/visx)
   └─ format.*(value, fieldConfig) ── presentation (units/dates), prefs-resolved  ← separate boundary
```

**Why backend is the right call (the user's React-Native point).** Transformations have the *same*
"N clients re-implement it and drift" property as formatting. If the pipeline is client TS, a React Native
app, an email renderer, and a webhook each need their own copy of `joinByField`/`calculateField`/`reduce` —
exactly the drift the platform mediates away for units/dates via `format.*`. Putting the pipeline in a host
verb (`viz.query`) backed by one Rust lib means **one implementation, every client thin** — consistent with
rule 7 (MCP is the contract) and the prefs doctrine. It runs symmetrically on edge and cloud (rule 1),
gated and workspace-walled (rules 5/6), over data already fetched through gated tools.

**This is not a new compute plane (rule 2).** `lb-viz` is a *pure library over data the host already
fetched through gated reads* — the same shape as `lb-prefs` being compiled in. It is **not** a new datastore,
not a forked authority, not a second query engine. The earlier scope rejected a "server-side transform
engine" on rule-2 grounds; that rejection was too strong — a pure host lib over bounded, already-authorized
rows fits the platform better than a TS-only lib that the next client must clone.

**Push-down still wins.** Single-target aggregation belongs in the query (SurrealQL/federation SQL); the
resolver only does what spans targets or SQL can't express (join across datasources, calculateField,
reduce-for-a-companion-panel). The resolver's frame budget + "push aggregation to the query/job" rule are
load-bearing — a target must not deliberately return a huge result for the host to crunch.

**Rejected alternative — client-side TS pipeline (the original decision).** Rejected: it forces every
non-web client (React Native, email, webhook, agent) to re-implement the transforms and drift, defeating
the very reason the platform mediates presentation. Instant editor preview (the one thing client-side buys)
is recovered by a debounced `viz.query` call on each edit — one implementation, slight latency, no fork.

## How it fits the core

- **Tenancy / isolation:** `viz.query` resolves the workspace from the **token**, never the panel spec;
  each target is dispatched workspace-walled exactly as a direct `bridge.call` would be. A ws-B panel can
  only ever resolve ws-B data. Mandatory two-session isolation test.
- **Capabilities:** **a new verb cap `mcp:viz.query:call`**, *and* the resolver composes each target tool's
  existing cap — it dispatches `store.query`/`series.*`/`federation.query`/an ext tool under
  `caller ∩ grant`, re-checked per target. A caller who lacks a target's cap gets that target denied
  (honest empty frame), not a bypass. The deny path is the headline test (a denied target → no rows; an
  ungranted `viz.query` → denied opaque).
- **Placement:** `either` — `lb-viz` is pure Rust compiled into every node; `viz.query` runs on edge and
  cloud identically. No role branch.
- **MCP surface (§6.1):**
  - **The add:** `viz.query(panel) -> { frames }` — a read/resolve verb (bounded, synchronous; the targets
    are each already bounded and the frame budget caps the pipeline). Not a batch.
  - **Live feed (fast-follow):** `viz.stream(panel) -> SSE<{frames}>` — the resolver subscribes to a live
    target's stream (the shipped series/bus SSE), folds each window, re-runs the pipeline, and pushes the
    resolved frames. Phase 1 ships the snapshot `viz.query`; `viz.stream` is the named live follow-up so a
    live panel doesn't re-transform client-side either.
  - **Consumed:** the target tools (`store.query`/`series.*`/`federation.query`/ext) — unchanged.
- **Data (SurrealDB):** only the **config** persists — `transformations[]` on the cell, bounded **≤32
  transforms/panel** (panel-model cap). Resolved frames are never stored.
- **Bus (Zenoh):** `viz.stream` rides the shipped SSE/motion; `viz.query` is a snapshot. State vs motion:
  the query/bus moves data, `lb-viz` shapes the snapshot, neither becomes the other.
- **Sync / authority:** `transformations[]` is part of the dashboard `(table,id)` UPSERT, serde-default
  additive, replays idempotently. The resolved data is not synced (it's derived).
- **Secrets:** none reach `lb-viz` — it sees only the already-fetched, already-redacted rows (a federation
  DSN stays inside the federation extension, never in a frame).
- **SDK/WIT impact — FLAG.** `viz.query`/`viz.stream` are **new host MCP verbs** (a long-lived contract:
  the panel→frames shape every client depends on). They are additive; pin the panel-spec input + the
  `Frame` output shape and version them. `lb-viz` is a new core crate (key-stack row). The `[[widget]]`/
  bridge contract is unchanged.

### The frame shape (canonical, columnar)

```
Frame  { refId?: string; name?: string; fields: Field[]; length: number }
Field  { name: string; type: "number"|"string"|"time"|"boolean"|"other"; values: [..canonical..]; labels?: {} }
```

Field-oriented (Grafana's `DataFrame`) so transforms port near-1:1. Values are **canonical** (UTC instants,
SI/base units) — `format.*` localizes at render. `Transformation { id, options?, disabled?, filter?: Matcher }`;
`filter`'s `Matcher` is the **same** shape `fieldConfig.overrides[]` uses
([`field-config-scope.md`](field-config-scope.md)) — one matcher model across the slice (in Rust here, in TS
for the editor; the shapes are mirrored and tested against the same cases).

### Phasing the transformer set

- **Phase 1 (high-value):** `reduce`, `organize` (rename/reorder/hide), `filterFieldsByName`,
  `filterByValue`, `sortBy`, `limit`, `groupBy`, `merge`/`joinByField`, `calculateField`.
- **Phase 2:** `renameByRegex`, `labelsToFields`, `configFromData`, `convertFieldType`,
  `formatTime`/`formatString`, `histogram`.
- **Deferred (named, not silent):** `spatial`, `partitionByValues`, `groupingToMatrix`, `regression`,
  `heatmap`, `transpose`, `joinByLabels`, `rowsToFields`, `extractFields`, `fieldLookup`, `seriesToRows`,
  `concatenate`, `ensureColumns`, `filterFrames`, `timeSeriesTable`, `noop`. An unsupported id is
  **preserved in config, marked disabled with a visible "unsupported — preserved, not applied" note** in the
  editor and skipped by the resolver — round-trip-faithful and honest.

### Interaction with `fieldConfig` and variables

- **Order:** the resolver runs targets → transforms → returns canonical frames; the **client** then applies
  `fieldConfig` formatting (units/decimals via `format.*`) and renders. Data shape (backend) precedes
  presentation (the prefs boundary) — never the reverse.
- **Variables:** a transform's `options` may reference dashboard variables (e.g. `filterByValue` threshold
  `$min`); the **caller passes the resolved `VarScope`** into `viz.query` (the host never resolves
  `${__user.*}` itself from a panel spec — identity comes from the token), and `lb-viz` interpolates option
  values before the step runs.

## Example flow

1. A React Native app opens a dashboard and calls `viz.query(panel)` for a panel with two targets — `A`
   (`store.query` over `readings`) and `B` (`federation.query` over Timescale). The host dispatches both
   under the app's token + grant, bounded, → two `Frame`s.
2. `lb-viz` runs `transformations[]`: `joinByField { byField:"ts" }` merges A+B on timestamp.
3. `calculateField { mode:"binary", left:"A.temp", op:"-", right:"B.temp", as:"delta" }` adds `delta`
   (canonical units).
4. `filterByValue { delta > $threshold }` (the app passed `$threshold` in the VarScope), then
   `sortBy`/`limit` bound the output. The host returns `{ frames }`.
5. The app renders the frame and calls `format.quantity` for the `delta` label in the user's prefs — the
   **web shell** does the *identical* two calls and shows the identical chart. Neither client re-implements
   the join or the unit math.
6. On import of a Grafana dashboard using `spatial` (deferred), the resolver skips it with the "unsupported
   — preserved, not applied" note; every other transform applies.

## Testing plan

Per [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — real gateway/store, seeded
real rows, **no `*.fake.ts`**. `lb-viz` is pure Rust → most cases are Rust unit tests over seeded frames;
end-to-end cases drive `viz.query` against a real spawned gateway.

- **Per-transform purity (Phase 1, each id) — Rust unit:** seed a known `Frame`, apply the transform,
  assert the exact output; determinism + no input mutation.
- **Ordered pipeline:** join → calculateField → filter → sort → limit over seeded A/B frames yields the
  expected final frame; reordering changes the result as Grafana would.
- **`disabled` + `filter`:** a disabled step is skipped but round-trips; a `filter` Matcher scopes a step.
- **Capability deny (mandatory):** `viz.query` denied without `mcp:viz.query:call`; a target the caller
  can't access is denied **inside** the resolver (honest empty frame, not a bypass) — assert the resolver
  cannot read what a direct call couldn't.
- **Workspace isolation (mandatory):** two real sessions — a ws-B `viz.query` resolves only ws-B targets;
  it can never name/read a ws-A datasource or series. Across store + MCP.
- **Canonical-only:** `viz.query` returns canonical values (no formatted strings); formatting is a separate
  `format.*` call (assert on the example path).
- **The bound (regression):** a panel whose targets hit their caps stays within the per-panel frame budget;
  an oversized intermediate frame is truncated-with-note or refused — never an unbounded loop.
- **Variable interpolation:** a `filterByValue` referencing a VarScope value resolves it; identity vars are
  never resolved from the panel spec (token-only).
- **End-to-end (real gateway):** a seeded `readings` table + a panel with a Phase-1 pipeline renders the
  resolved frames through `viz.query` → render; the **same** call from a second (thin) client yields the
  same frames (the multi-client-identity test — the reason this is backend).

## Risks & hard problems

- **The frame budget is the whole game.** `viz.query` must cap intermediate frame size and push aggregation
  to the query/job, or a panel with a huge target hangs the host. Enforce in the resolver, surface in the
  editor.
- **The resolver must dispatch targets under the caller's authority, not its own.** A bug that runs a target
  with host privilege instead of `caller ∩ grant` is a privilege escalation. Re-check each target's cap +
  the workspace per dispatch; the deny test must bite a real ungranted target.
- **Rust/TS matcher parity.** The `Matcher` (transform `filter`, fieldConfig override) exists in Rust
  (`lb-viz`) and TS (the editor preview UI). Mirror the shapes and test both against the same fixtures, or
  preview and resolve disagree.
- **`calculateField` bounded modes only.** No arbitrary eval in the host — binary/reduce/index/unary,
  validated; anything richer is a query or a job.
- **Live recompute cost (`viz.stream`).** Re-running the pipeline per SSE window can be hot; keep Phase-1
  transforms O(n) over the bounded window and memoize on `(frames, transformations[])` identity in the
  resolver.
- **`viz.query` is a forever contract.** The panel-spec input + `Frame` output are depended on by every
  client; version them and keep additive.

## Resolved decisions

No blocking open questions — these are the long-term answers the build follows.

- **Transformations run in the backend** — a pure Rust `lb-viz` lib behind a `viz.query(panel) -> {frames}`
  verb — so every client (web, **React Native**, email, webhook) renders identical data without
  re-implementing the pipeline. Client-side TS transforms are **rejected** (multi-client drift).
- **`viz.query` returns strictly canonical frames; `format.*` stays a separate presentation call.** Data
  shape (backend resolver) and presentation (prefs boundary) are distinct verbs — the thinnest client gets
  both backend-mediated, but they don't conflate.
- **The resolver dispatches targets under `caller ∩ grant`, workspace from the token**, composing each
  target tool's existing cap with the new `mcp:viz.query:call`. No new query semantics; SurrealDB stays
  authority; push-down to the query/job is still preferred for heavy aggregation.
- **Columnar `Frame` is the canonical type** (matches Grafana; transforms port cleanly); the row↔frame
  adapter lives at the resolver's edges.
- **The per-panel frame budget is enforced in the resolver, once.** `topic` is kept in config for
  round-trip, deferred on apply (no annotations plane yet). No separate transform version field (rides the
  cell `v` / dashboard `schemaVersion`).
- **`viz.stream` (live frames over SSE) is the named follow-up** so live panels don't re-transform
  client-side either; Phase 1 ships the snapshot `viz.query`.

## Related

- [`README.md`](README.md) — the viz umbrella (the reconciliation table; the backend-resolve decision).
- [`panel-model-scope.md`](panel-model-scope.md) — the spine; declares `Transformation` + `sources[]` that
  `viz.query` composes, and the ≤32/panel bound.
- [`field-config-scope.md`](field-config-scope.md) — the `format.*` presentation boundary that runs **after**
  `viz.query`, and the shared `Matcher` shape.
- [`chart-types-scope.md`](chart-types-scope.md) — the renderers the resolved frames feed.
- [`datasource-binding-scope.md`](datasource-binding-scope.md) — the targets `viz.query` dispatches.
- [`import-export-scope.md`](import-export-scope.md) — maps Grafana `transformations[]` 1:1 and drives the
  preserve-but-disable degradation.
- [`panel-editor-scope.md`](panel-editor-scope.md) — the Transform tab; preview is a debounced `viz.query`.
- [`../../../prefs/user-prefs-scope.md`](../../../prefs/user-prefs-scope.md) — the structural twin
  (`lb-prefs`: pure lib + MCP verbs, thin clients free) and the `format.*` boundary.
- [`../../../jobs/jobs-scope.md`](../../../jobs/jobs-scope.md) — where heavy aggregation goes instead of an
  oversized resolver frame.
- [`../../../testing/testing-scope.md`](../../../testing/testing-scope.md) — real-gateway/no-fakes doctrine.
- The Grafana reference clone at `/tmp/grafana` —
  `packages/grafana-data/src/transformations/transformers/ids.ts` (the id set + `DataTransformerConfig`).
- `key-stack.md` — adds the **`lb-viz`** crate row (pure transform lib, the data-resolve twin of `lb-prefs`).
- README **§6.1** (API shape/bounds), **§6.5/§3.7** (MCP as the contract), **§6.10** (jobs), **§3** (rules
  1/2/3/5/6/7).
