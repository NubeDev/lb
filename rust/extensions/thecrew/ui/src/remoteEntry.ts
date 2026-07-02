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

/** The federated PAGE contract: inject styles once, then mount the graphics page. The shell's
 *  `loadRemoteMount`/`pickMount` resolves the page by the name **`mount`** (the frozen federation
 *  contract, byte-for-byte proof-panel's) — NOT `mountPage`. Exporting only `mountPage` made the live
 *  shell throw "remote does not export a `mount` function" and the page never mounted. `mountPage`
 *  stays as an alias so the existing unit tests (which import it) keep passing. */
export function mount(el: HTMLElement, ctx: MountCtx, bridge: Bridge): () => void {
  injectStyles();
  return mountPageImpl(el, ctx, bridge);
}

/** Back-compat alias for `mount` — the unit suite imports `mountPage`. */
export const mountPage = mount;

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

export default { mount, mountWidget };
