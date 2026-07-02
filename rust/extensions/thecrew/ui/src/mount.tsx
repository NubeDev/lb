// The federation mount contract (ui-federation scope): the shell dynamic-imports this remote
// (sharing its React via the host import map) and calls `mountPage`/`mountWidget`. One build, two
// exports on the same `remoteEntry.js` (proof-panel's precedent). Data is reached ONLY through the
// bridge; neither mount ever sees a token, DB, or fetch.

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { ScenePage } from "./bridge/ScenePage";
import { SceneWidget } from "./bridge/SceneWidget";
import { useSceneStore } from "./state/scene-store";
import type { Bridge, MountCtx, WidgetBridge, WidgetCtx } from "./bridge/contract";

/**
 * Mount the full graphics PAGE into `el`. The shell reaches this through the federation entry
 * (`remoteEntry.ts`, which injects compiled CSS first), sharing its React singletons.
 */
export function mountPage(el: HTMLElement, _ctx: MountCtx, bridge: Bridge): () => void {
  // Live-browser test seam (graphics-canvas phases 1–2 e2e): expose the scene store on the page so a
  // Playwright spec can `select`/`nudge` a shape deterministically — WebGL pointer-picking is
  // unreliable headless, and this is the SAME store the editor writes through (a UI selection handle,
  // not a backend fake: the actual persistence still flows through the real `assets.put_doc` bridge).
  (window as unknown as { __tcStore?: typeof useSceneStore }).__tcStore = useSceneStore;
  const root = createRoot(el);
  root.render(
    <StrictMode>
      <ScenePage bridge={bridge} />
    </StrictMode>,
  );
  return () => root.unmount();
}

/**
 * Mount a read-only dashboard scene CELL. `ctx.options.sceneId` (or `ctx.binding.sceneId`) selects
 * the scene; the widget renders it read-only over the (narrower) widget bridge. `widgetId` selects
 * which `[[widget]]` tile — this ext ships one ("scene").
 */
export function mountWidget(
  el: HTMLElement,
  ctx: WidgetCtx,
  bridge: WidgetBridge,
  _widgetId: string,
): () => void {
  const root = createRoot(el);
  const sceneId =
    (ctx.options?.sceneId as string | undefined) ??
    (ctx.binding?.sceneId as string | undefined) ??
    (ctx.vars?.sceneId as string | undefined) ??
    "";
  root.render(
    <StrictMode>
      <SceneWidget bridge={bridge} sceneId={sceneId} />
    </StrictMode>,
  );
  return () => root.unmount();
}
