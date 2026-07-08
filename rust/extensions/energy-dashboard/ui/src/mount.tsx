import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./styles/main.css";
import { App } from "./App";
import { setBridge } from "./bridge";

/*
 * Mount the extension page. The shell dynamic-imports this remote (`loadRemoteMount` in
 * `ui/src/features/ext-host/federation.ts`) and calls `mount(el, ctx, bridge)` with:
 *   - `el`: the DOM element to render into
 *   - `ctx`: `{ workspace: string }` — the workspace id
 *   - `bridge`: `{ call: <T>(tool, args?) => Promise<T> }` — the ONLY way to reach host tools
 *
 * The bridge calls `POST /mcp/call` under the session token; the host re-checks caps per call.
 * The extension never sees a token, DB, or fetch — ALL data flows through `bridge.call`.
 *
 * The root `.lbx-energy-dashboard` wrapper anchors the scoped theme tokens (tokens.css). Keep it even when
 * rewriting the page contents.
 */
export function mount(
  el: HTMLElement,
  _ctx: { workspace: string },
  b: { call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T> },
): () => void {
  setBridge(b);
  const root = createRoot(el);
  root.render(
    <StrictMode>
      <div className="lbx-energy-dashboard">
        <App />
      </div>
    </StrictMode>,
  );
  return () => root.unmount();
}
