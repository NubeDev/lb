// The remote entry the shell dynamic-imports from `GET /extensions/control-engine/ui/remoteEntry.js`
// (ui-federation scope). Re-exports the frozen `mount(el, ctx, bridge)` contract, wrapped so BOTH the
// page's own compiled Tailwind CSS AND the vendored CeEditor's bundled stylesheet are injected into
// <head> the first time the remote loads — the lib build emits a single JS file (cssCodeSplit off), so
// styles ship as `?inline` strings and are injected at runtime, not bundled by the shell.
//
// `react` (+ the other React entry points) are externalised by the build, so the bare imports inside
// `mount`/`Page`/the vendored editor resolve through the shell's import map to the host's SINGLE React.

import styles from "@/styles/tokens.css?inline";
// The vendored editor's bundled theme (built to `dist/ce-wiresheet.css`, aliased in vite.config.ts).
// Imported `?raw` (NOT `?inline`) so our tailwind-v3 PostCSS pipeline does NOT re-process an
// already-compiled tailwind-v4 stylesheet — we inject its bytes verbatim, exactly as built.
import editorStyles from "@nube/ce-wiresheet/style.css?raw";
import { mount as mountImpl } from "@/mount";
import type { Bridge, MountCtx } from "@/contract";

let injected = false;
function injectStyles() {
  if (injected || typeof document === "undefined") return;
  injected = true;
  for (const [scope, css] of [
    ["control-engine", styles],
    ["control-engine-editor", editorStyles],
  ] as const) {
    const el = document.createElement("style");
    el.dataset.ext = scope;
    el.textContent = css;
    document.head.appendChild(el);
  }
}

/** The single exported federation contract. The shell loads this remote (sharing its React) and calls
 *  `mount(el, ctx, bridge)`; we inject the page + editor styles once, then delegate to the React root. */
export function mount(el: HTMLElement, ctx: MountCtx, bridge: Bridge): () => void {
  injectStyles();
  return mountImpl(el, ctx, bridge);
}

export default { mount };
