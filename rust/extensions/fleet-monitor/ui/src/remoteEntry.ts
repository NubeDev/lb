// The remote entry the shell dynamic-imports from `GET /extensions/fleet-monitor/ui/remoteEntry.js`
// (ui-federation scope). It re-exports the frozen `mount(el, ctx, bridge)` contract the shell calls,
// wrapped so the bundle's compiled Tailwind CSS is injected into <head> the first time the remote
// loads.
//
// `react` (and the other React entry points) are externalised by the build, so the bare imports inside
// `mount`/`App`/the pages resolve through the shell's import map to the host's SINGLE React. The
// compiled CSS is imported `?inline` (a string) — `tokens.css` carries the `@tailwind` directives, so
// this is the whole page's styling — and injected once, so the page is styled without the shell
// bundling it.

import styles from "@/styles/tokens.css?inline";
import { mount as mountImpl } from "@/mount";
import type { Bridge, MountCtx } from "@/app/contract";

let injected = false;
function injectStyles() {
  if (injected || typeof document === "undefined") return;
  injected = true;
  const el = document.createElement("style");
  el.dataset.ext = "fleet-monitor";
  el.textContent = styles;
  document.head.appendChild(el);
}

/** The single exported federation contract. The shell loads this remote (sharing its React) and calls
 *  `mount(el, ctx, bridge)`; we inject the page's styles once, then delegate to the React root mount. */
export function mount(el: HTMLElement, ctx: MountCtx, bridge: Bridge): () => void {
  injectStyles();
  return mountImpl(el, ctx, bridge);
}

export default { mount };
