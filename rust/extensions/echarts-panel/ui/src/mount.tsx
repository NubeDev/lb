import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "@/App";
import { mountChart, type ChartCtx, type TileHandle } from "@/chart/mountChart";
import type { Bridge, MountCtx } from "@/app/contract";

/**
 * Render the (trivial) page into `el` with `createRoot` and return an unmount cleanup. The shell reaches
 * this through the federation entry (`remoteEntry.ts`, which injects the compiled CSS first), sharing its
 * React singletons. The page is deliberately minimal — it just states the extension is installed; the
 * real surface is the `chart` widget. Data (there is none for the page) would be reached ONLY through
 * `bridge`; the page never sees a token, DB, or fetch.
 */
export function mount(el: HTMLElement, _ctx: MountCtx, _bridge: Bridge): () => void {
  const root = createRoot(el);
  root.render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  return () => root.unmount();
}

/**
 * Mount a dashboard WIDGET tile (widget-builder scope) — the SECOND named export the shell's
 * `ext:<id>/<widget>` renderer calls on the SAME remote (one build, one remoteEntry). This extension
 * ships ONE `[[widget]]`: label "Chart" → slug `chart`.
 *
 * CRITICAL — this is a DATA tile (`[[widget]].data = true`). It renders `ctx.data` (a Frame[]) +
 * `ctx.fieldConfig`; it NEVER calls the bridge for data (the SHELL fetches via `viz.query` and pushes
 * fresh frames in). The `bridge` arg is accepted for signature compatibility but unused by this tile.
 *
 * Return shape: for the `chart` slug we return the DATA-TILE lifecycle OBJECT `{ update, teardown }`
 * (from `mountChart`) so the shell can push fresh frames without a remount. For an unknown slug we return
 * a bare unmount `() => void` (nothing to update). The shell distinguishes them by shape.
 *
 * Version gate: `mountChart` inspects `ctx.v` — `>= 3` renders frames; older hosts get an honest
 * "needs a frames-capable (v3) host" message rather than a fabricated series.
 */
export function mountWidget(
  el: HTMLElement,
  ctx: ChartCtx,
  _bridge: unknown,
  widgetId: string,
): TileHandle | (() => void) {
  if (widgetId === "chart") {
    return mountChart(el, ctx);
  }
  // Unknown slug: honest empty mount (no fabricated tile), a bare unmount so the shell can still clean up.
  const root = createRoot(el);
  root.render(
    <StrictMode>
      <div className="flex h-full w-full items-center justify-center p-2">
        <span className="text-xs text-muted">unknown widget: {widgetId}</span>
      </div>
    </StrictMode>,
  );
  return () => root.unmount();
}
