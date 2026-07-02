import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { Page } from "./Page";
import type { Bridge, MountCtx } from "./contract";

/**
 * Render the page into `el` with `createRoot` and return an unmount cleanup. The shell reaches this
 * through the federation entry (`remoteEntry.ts`, which injects the page's + the vendored editor's CSS
 * first), sharing its React singletons. Data is reached ONLY through `bridge` (the caps-gated MCP seam);
 * the page never sees a token, DB, fetch, or a CE socket. (Styles are NOT imported here — `remoteEntry`
 * injects them `?inline` so the lib build emits a single JS file.)
 */
export function mount(el: HTMLElement, ctx: MountCtx, bridge: Bridge): () => void {
  const root = createRoot(el);
  root.render(
    <StrictMode>
      <Page ctx={ctx} bridge={bridge} />
    </StrictMode>,
  );
  return () => root.unmount();
}
