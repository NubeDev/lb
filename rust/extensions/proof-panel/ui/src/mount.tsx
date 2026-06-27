import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "@/App";
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
