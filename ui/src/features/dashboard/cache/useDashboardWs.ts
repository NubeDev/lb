// The dashboard's workspace, in context (dashboard-query-cache-scope). Every read-cache key is
// ws-prefixed for tenant isolation, but the deep read hooks (`useVizQuery`, `useSeries`,
// `useFlowNodeValue`) don't take `ws` as a prop — they render inside cells nested well below the page.
// `DashboardCacheProvider` puts the current `ws` here once (sourced from the page's `ws` prop) so any
// hook reads it without prop-drilling. A workspace switch remounts the provider with a new `ws` → new
// keys → no cross-ws cache bleed (the host still re-checks the ws from the token; the key is de-dup).

import { createContext, useContext } from "react";

/** The dashboard workspace context. `null` outside a `DashboardCacheProvider` — a caller that reads it
 *  without the provider is a wiring bug, so we throw rather than silently key everything under "". */
export const DashboardWsContext = createContext<string | null>(null);

/** The current dashboard workspace. Throws if read outside `DashboardCacheProvider` (a wiring bug — a
 *  read hook must never fall back to an unscoped key that would bleed across workspaces). */
export function useDashboardWs(): string {
  const ws = useContext(DashboardWsContext);
  if (ws === null) throw new Error("useDashboardWs: no DashboardCacheProvider in tree");
  return ws;
}

/** The current dashboard workspace, or `null` outside a `DashboardCacheProvider`. For a consumer that
 *  is ALSO valid without the cache (an ext widget may mount standalone — a v2 self-fetching tile needs
 *  no frames): it reads the ws when present and does no cache read when absent. A DATA tile only reaches
 *  its `viz.query` when a provider supplies the ws, so this never keys under an unscoped ws. */
export function useDashboardWsOptional(): string | null {
  return useContext(DashboardWsContext);
}
