import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./styles/main.css";
import { App } from "./App";
import { setBridge } from "./bridge";

export function mount(
  el: HTMLElement,
  _ctx: { workspace: string },
  b: { call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T> },
): () => void {
  setBridge(b);
  const root = createRoot(el);
  root.render(
    <StrictMode>
      <div className="lbx-test-panel">
        <App />
      </div>
    </StrictMode>,
  );
  return () => root.unmount();
}
