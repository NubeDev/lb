// The dashboard read cache's provider (dashboard-query-cache-scope). It gives the dashboard subtree two
// things, both scoped to the VISIT: a per-mount react-query `QueryClient` (minted once, dropped on
// unmount — so the cache lives "while on the dashboard page" and clears on navigate-away, the scope's
// "clear on leave"), and the current `ws` in context (every cache key is ws-prefixed; the deep read hooks
// read it here instead of prop-drilling). `DashboardView` wraps its own body with this, so the real route
// AND the gateway tests (which render `DashboardView` directly) both get the cache with no extra wiring.
// A workspace switch remounts with a new `ws` → new keys → no cross-ws bleed. One responsibility: scope
// the dashboard read cache to one workspace visit.

import { useState, type ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";

import { makeDashboardQueryClient } from "./dashboardQueryClient";
import { DashboardWsContext } from "./useDashboardWs";

/** Provide the per-visit `QueryClient` + the current `ws` to the dashboard subtree. Keyed by the caller
 *  on `ws` (see `DashboardView`) so a workspace switch remounts with a fresh client and fresh keys. */
export function DashboardCacheProvider({ ws, children }: { ws: string; children: ReactNode }) {
  const [client] = useState(makeDashboardQueryClient);
  return (
    <DashboardWsContext.Provider value={ws}>
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    </DashboardWsContext.Provider>
  );
}
