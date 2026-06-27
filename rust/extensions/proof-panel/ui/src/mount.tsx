import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "@/App";
import type { Bridge, MountCtx } from "@/app/contract";
// Co-locate the token CSS with the federated entry so styling works whether served by the gateway or
// in dev. The bundle ships these tokens; mirrored from the shell so the page looks native.
import "@/styles/tokens.css";

/**
 * The single exposed federation module (`./mount`). The shell loads this remote (sharing its React
 * singletons) and calls `mount(el, ctx, bridge)`. We render into `el` with `createRoot` and return an
 * unmount cleanup. Data is reached ONLY through `bridge`; the page never sees a token, DB, or fetch.
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
