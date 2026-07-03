// The remote entry the shell dynamic-imports from `GET /extensions/echarts-panel/ui/remoteEntry.js`
// (ui-federation scope). It re-exports the frozen `mount(el, ctx, bridge)` PAGE contract and the
// `mountWidget(el, ctx, bridge, widgetId)` WIDGET contract, wrapped so the bundle's compiled Tailwind
// CSS is injected into <head> the first time the remote loads.
//
// `react` (and the other React entry points) are externalised by the build, so the bare imports inside
// `mount`/`App`/`ChartTile` resolve through the shell's import map to the host's SINGLE React. `echarts`
// is BUNDLED (not externalised), so the chart tile is self-contained. The compiled CSS is imported
// `?inline` (a string) — `tokens.css` carries the `@tailwind` directives — and injected once, so the
// page + tile are styled without the shell bundling it.

import styles from "@/styles/tokens.css?inline";
import { mount as mountImpl, mountWidget as mountWidgetImpl } from "@/mount";
import type { Bridge, MountCtx } from "@/app/contract";
import type { ChartCtx, TileHandle } from "@/chart/mountChart";

let injected = false;
function injectStyles() {
  if (injected || typeof document === "undefined") return;
  injected = true;
  const el = document.createElement("style");
  el.dataset.ext = "echarts-panel";
  el.textContent = styles;
  document.head.appendChild(el);
}

/** The federation PAGE contract. The shell loads this remote (sharing its React) and calls
 *  `mount(el, ctx, bridge)`; we inject the page's styles once, then delegate to the React root mount. */
export function mount(el: HTMLElement, ctx: MountCtx, bridge: Bridge): () => void {
  injectStyles();
  return mountImpl(el, ctx, bridge);
}

/** The dashboard WIDGET contract (widget-builder scope) — a SECOND named export on the same remote the
 *  shell's `ext:<id>/<widget>` renderer calls. Injects styles once, then mounts the tile. For the `chart`
 *  DATA tile this returns the frames-in lifecycle object `{ update, teardown }` so the shell can push
 *  fresh frames without a remount; an unknown slug returns a bare unmount. */
export function mountWidget(
  el: HTMLElement,
  ctx: ChartCtx,
  bridge: unknown,
  widgetId: string,
): TileHandle | (() => void) {
  injectStyles();
  return mountWidgetImpl(el, ctx, bridge, widgetId);
}

export default { mount, mountWidget };
