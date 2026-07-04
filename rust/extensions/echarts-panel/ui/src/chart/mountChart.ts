// The imperative handler behind the `chart` widget slug. It createRoots `ChartTile` and returns the
// DATA-TILE lifecycle object `{ update, teardown }` — an OBJECT, not a bare unmount fn — so the shell
// can push FRESH frames into an already-mounted tile without a remount:
//   • `update(ctx)` re-renders `ChartTile` with the new `ctx.data`/`ctx.fieldConfig` (React reconciles
//     the ECharts `setOption` in-place — no root teardown, no chart re-init, no flicker);
//   • `teardown()` unmounts the root (which disposes the ECharts instance via the tile's cleanup).
//
// CONTRACT (ctx v4 — frames-in + theme; see SCOPE.md "Assumed ctx shape"):
//   ctx.v >= 3, ctx.data: Frame[], ctx.fieldConfig?: FieldConfig, ctx.workspace: string;
//   ctx.v >= 4 ALSO carries ctx.theme (resolved tokens) — the reference consumer of the live-re-theme
//   contract. ECharts can't read a CSS var, so it recolors from ctx.theme.chart/accent/fg on every
//   `update(ctx)` the shell fires on a theme change — no re-mount. The tile is pure-render: it reads ONLY
//   ctx.data/ctx.fieldConfig/ctx.theme, never a bridge, token, or fetch. On a host that predates v3
//   (`ctx.v < 3`) there are no frames — we show an honest "requires a frames-capable (v3) host" message.

import { createElement } from "react";
import { createRoot, type Root } from "react-dom/client";

import { ChartTile } from "./ChartTile";
import type { Frame, FieldConfig } from "./frame.types";

/** The resolved theme tokens the shell passes at ctx v4 (subset the chart uses; the full shape is the
 *  host's WidgetTheme). Additive — absent on a v3 host, in which case the chart uses ECharts defaults. */
export interface ChartTheme {
  fg?: string;
  muted?: string;
  border?: string;
  accent?: string;
  panel?: string;
  chart?: string[];
}

/** The frames-in widget context the shell pushes into a `data = true` tile. Additive: older hosts omit
 *  `v`/`data`/`theme`; the handler version-gates on `v >= 3` (frames) and reads `theme` when present (v4). */
export interface ChartCtx {
  v?: number;
  workspace?: string;
  data?: Frame[];
  fieldConfig?: FieldConfig;
  /** v4: resolved theme tokens. ECharts recolors from these (it can't read a CSS var). */
  theme?: ChartTheme;
  // Older-host fields (v2 binding/options) may still ride along; the chart tile ignores them.
  binding?: Record<string, unknown>;
  options?: Record<string, unknown>;
}

/** The data-tile lifecycle the shell drives. `update` swaps in fresh frames without remount; `teardown`
 *  disposes everything. This is the object shape the shell's `ext:<id>/<widget>` renderer expects for a
 *  `data = true` tile. */
export interface TileHandle {
  update: (ctx: ChartCtx) => void;
  teardown: () => void;
}

/** True when the host speaks the v3 frames contract. */
function isFramesHost(ctx: ChartCtx): boolean {
  return typeof ctx.v === "number" && ctx.v >= 3;
}

/** Render the chart tile for a given ctx: frames when the host is v3, else an honest message element. */
function renderFor(ctx: ChartCtx) {
  if (!isFramesHost(ctx)) {
    return createElement(
      "div",
      {
        className: "flex h-full w-full items-center justify-center p-2",
        "data-echarts-state": "needs-v3",
      },
      createElement(
        "span",
        { className: "text-xs text-muted" },
        "this chart tile needs a frames-capable (v3) host",
      ),
    );
  }
  return createElement(ChartTile, { frames: ctx.data ?? [], fieldConfig: ctx.fieldConfig, theme: ctx.theme });
}

/** createRoot the chart tile and return `{ update, teardown }`. `update` re-renders the SAME root with
 *  fresh frames (React reconciles → ECharts `setOption` in place); `teardown` unmounts (disposing the
 *  ECharts instance through the tile's own cleanup). */
export function mountChart(el: HTMLElement, ctx: ChartCtx): TileHandle {
  const root: Root = createRoot(el);
  root.render(renderFor(ctx));
  return {
    update(next: ChartCtx) {
      root.render(renderFor(next));
    },
    teardown() {
      root.unmount();
    },
  };
}
