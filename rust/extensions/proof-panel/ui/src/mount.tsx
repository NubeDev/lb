import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "@/App";
import { WidgetTile, type WidgetBridge } from "@/app/WidgetTile";
import type { Bridge, MountCtx } from "@/app/contract";

/**
 * Render the page into `el` with `createRoot` and return an unmount cleanup. The shell reaches this
 * through the federation entry (`remoteEntry.ts`, which injects the page's compiled CSS first), sharing
 * its React singletons. Data is reached ONLY through `bridge`; the page never sees a token, DB, or
 * fetch. (Styles are NOT imported here — `remoteEntry.ts` injects them `?inline` so the lib build emits
 * a single JS file; the standalone `dev.tsx` harness imports `tokens.css` itself.)
 */
export function mount(el: HTMLElement, ctx: MountCtx, bridge: Bridge): () => void {
  const root = createRoot(el);
  root.render(
    <StrictMode>
      <App ctx={ctx} bridge={bridge} />
    </StrictMode>,
  );
  return () => root.unmount();
}

/**
 * Mount the dashboard WIDGET tile (widget-builder scope) — the SECOND named export the dashboard's
 * `ext:<id>/<widget>` renderer calls on the SAME remote (follow-up 2: one build, one remoteEntry). The
 * shell passes the v2 widget `ctx`/`bridge` (the bridge may `call` and `watch`) and the `widgetId`
 * naming which `[[widget]]` tile to render (this ext ships one, so it ignores it). Reaches only its
 * `[[widget]].scope ∩ grant`, re-checked at the host — never a token.
 */
export function mountWidget(
  el: HTMLElement,
  _ctx: { workspace: string; binding: Record<string, unknown>; options: Record<string, unknown> },
  bridge: WidgetBridge,
  _widgetId: string,
): () => void {
  const root = createRoot(el);
  root.render(
    <StrictMode>
      <WidgetTile bridge={bridge} />
    </StrictMode>,
  );
  return () => root.unmount();
}
