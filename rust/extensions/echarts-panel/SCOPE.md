# echarts-panel — scope: the frames-in reference DATA TILE

**What this extension is for.** `echarts-panel` is the Tier-1 WASM reference **data tile**. Its *whole
point* is ONE dashboard widget — a chart rendered with Apache ECharts — that proves ONE render path:
**frames in (`ctx.data`) + the shared Field-tab options (`ctx.fieldConfig`) → a chart**, working
identically on every surface that can hand a tile frames (dashboard cells today; channel surfaces next).
It is a companion to `proof-panel` (the "whole platform, one page" demo); where proof-panel exercises
*bridge calls*, echarts-panel exercises the *pushed-frames* contract.

This file is the **co-located scope** (the ask). Session log:
`docs/sessions/extensions/echarts-panel-session.md`. Public truth:
`docs/public/extensions/extensions.md`.

---

## The shape (cloned from proof-panel's build shape)

- **Backend** — a trivial wasm32-wasip2 component (`src/lib.rs`) serving ONE static tool,
  `echarts.about` → `{ ok: true, ext: "echarts-panel" }`. It exists only to prove the Tier-1 backend
  half is a real, reachable MCP tool (the WASM analogue of `proof.ping`, minus any echo). The chart tile
  does **not** bind to it.
- **Frontend** — a module-federated remote (`ui/`, Vite lib build → `dist/remoteEntry.js`) exposing:
  - `mount(el, ctx, bridge)` — a trivial "installed" page (`App.tsx`);
  - `mountWidget(el, ctx, bridge, widgetId)` — for slug `chart`, the frames-in ECharts tile.

## The frames-in contract (the new part)

The `[[widget]]` block carries **`data = true`** — the frames-in opt-in. That tells the shell this tile
consumes `ctx.data`, so the SHELL runs `viz.query` and pushes fresh frames into the tile. Because the
shell fetches, the tile needs **no read verbs**: `scope = []`, `[capabilities].request = []`.

**The tile is a pure renderer.** It NEVER fetches, never sees a token or DB, never calls the bridge for
data — it renders only `ctx.data` + `ctx.fieldConfig`. Statelessness is trivial (no instance state).

### Assumed `ctx` v3 shape (RECONCILE WITH HOST)

The widget handler (`chart/mountChart.ts`) assumes this frames-in ctx and **version-gates on `ctx.v`**:

```ts
interface ChartCtx {
  v: 3;                    // frames-in host; < 3 (or absent) → honest "needs a v3 host" message
  workspace: string;
  data: Frame[];           // the frames the shell fetched via viz.query and pushed in
  fieldConfig?: FieldConfig;
}
```

`Frame` (mirrors `ui/src/features/dashboard/builder/useVizQuery.ts`):

```ts
interface Field { name: string; type?: string; values: unknown[] }
interface Frame { refId?: string; name?: string; fields: Field[]; length?: number }
```

`FieldConfig` subset (mirrors `ui/src/lib/dashboard/fieldconfig.types.ts` `FieldOptions`):
`defaults?: { displayName?, unit?, decimals?, min?, max?, thresholds?, custom? }`.

### Return shape (RECONCILE WITH HOST)

For the `chart` slug, `mountWidget` returns the **data-tile lifecycle OBJECT** (not a bare unmount fn):

```ts
interface TileHandle { update(ctx: ChartCtx): void; teardown(): void }
```

`update(ctx)` re-renders the tile with fresh `ctx.data`/`ctx.fieldConfig` (React reconciles →
ECharts `setOption`, no remount); `teardown()` unmounts (disposing the ECharts instance). An unknown
slug returns a bare `() => void`.

## Mapping (framesToOption — the pure piece)

`framesToOption(frames, fieldConfig): EChartsOption` (pure, tested):
- X axis = first `time`-typed field, else the first field, of the primary frame.
- One series per numeric non-axis field across all frames (line by default; `custom.drawStyle==="bar"` → bar).
- `unit`/`decimals` format the y-axis label + tooltip; `thresholds.steps` → y-axis `markLine`s;
  legend shown when >1 series or `custom.showLegend===true`.
- Honest states: no frames / all-empty → "no data"; an error-shaped frame → its message. Never a fake series.

## FILE-LAYOUT (one responsibility per file)

`ui/src/chart/frame.types.ts` (local Frame/FieldConfig subset), `framesToOption.ts` (pure map),
`ChartTile.tsx` (React ECharts renderer), `mountChart.ts` (imperative `{update,teardown}` handler).
`ui/src/mount.tsx` dispatches the slug; `remoteEntry.ts` injects CSS + re-exports. ≤400 lines/file,
no `utils`/`helpers`.

## Testing plan

- **Rust unit** (`src/lib.rs`): `echarts.about` ok, unknown-tool errors, bad-params errors.
- **UI unit** (`framesToOption.test.ts`): the mapping honours unit/decimals/thresholds/drawStyle/legend,
  maps empty → no series, never plots string fields (no fake series).
- **Follow-up (host-side):** once the v3 ctx contract lands, a gateway/dashboard test that seeds a real
  series, lets the shell run `viz.query`, and asserts the tile renders the pushed frames (real path, no fake).

## Non-negotiables held

- Stateless tile (no durable state). Pure-render (no fetch, no token, no DB).
- Additive contract: version-gate on `ctx.v >= 3`.
- `echarts` is bundled into the remote (not externalised); React stays externalised (host singleton).
- No core branch on this extension id — reached only through the generic `ext:<id>/<widget>` seam.
