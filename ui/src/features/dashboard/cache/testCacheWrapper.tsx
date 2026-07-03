// Test-only wrapper: render a dashboard sub-component (a panel view, an editor tab, a picker) INSIDE the
// dashboard read cache it now depends on (dashboard-query-cache-scope). The real surface gets the cache
// from `DashboardCacheProvider` (mounted by `DashboardView`/`ResponseView`); a gateway test that renders a
// sub-component in isolation must supply the same boundary — this is that boundary. It mints a FRESH
// QueryClient per render (via the provider's own per-mount client) so cache state never leaks between
// tests, and takes the workspace so keys are ws-prefixed exactly as in production.

import type { ReactNode } from "react";

import { DashboardCacheProvider } from "./DashboardQueryProvider";

/** Wrap `children` in the dashboard read cache for a test render. Pass the test's workspace so cache keys
 *  match production; the provider mints a per-mount client so each test starts cold. */
export function WithDashboardCache({ ws, children }: { ws: string; children: ReactNode }) {
  return <DashboardCacheProvider ws={ws}>{children}</DashboardCacheProvider>;
}
