// The remote entry the shell dynamic-imports from `GET /extensions/thecrew/ui/remoteEntry.js`
// (ui-federation scope). It re-exports the `mountPage`/`mountWidget` contract the shell calls,
// wrapped so the bundle's compiled Tailwind CSS is injected into <head> the first time the remote
// loads. `react` (+ the other React entry points) are externalised by the build, so the bare
// imports inside the mounts resolve through the shell's import map to the host's SINGLE React —
// no second copy, no "Invalid hook call". three.js DOES ride this remote (the federation payoff:
// only this bundle carries the engine — thecrew-extension-scope.md §Risks/Bundle weight).

import styles from "./styles.css?inline";
import { mountPage as mountPageImpl, mountWidget as mountWidgetImpl } from "./mount";
import type { Bridge, MountCtx, WidgetBridge, WidgetCtx } from "./bridge/contract";

let injected = false;
function injectStyles() {
  if (injected || typeof document === "undefined") return;
  injected = true;
  const el = document.createElement("style");
  el.dataset.ext = "thecrew";
  el.textContent = styles;
  document.head.appendChild(el);
}

/** The federated PAGE contract: inject styles once, then mount the graphics page. */
export function mountPage(el: HTMLElement, ctx: MountCtx, bridge: Bridge): () => void {
  injectStyles();
  return mountPageImpl(el, ctx, bridge);
}

/** The dashboard WIDGET contract (a SECOND named export on the same remote): read-only scene cell. */
export function mountWidget(
  el: HTMLElement,
  ctx: WidgetCtx,
  bridge: WidgetBridge,
  widgetId: string,
): () => void {
  injectStyles();
  return mountWidgetImpl(el, ctx, bridge, widgetId);
}

export default { mountPage, mountWidget };
