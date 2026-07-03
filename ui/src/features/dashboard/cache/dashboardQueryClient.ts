// The dashboard read cache's QueryClient (dashboard-query-cache-scope). ONE `QueryClient` per dashboard
// visit — the `DashboardQueryProvider` mints it and drops it on route leave, so the cache lives "while
// the user is on the dashboard page" and is torn down when they navigate away (the scope's "clear on
// leave"). This is the ONLY react-query client on the surface; every dashboard read hook keys into it.
//
// Defaults are the de-dup knobs, not per-query freshness. `staleTime` here is the FLOOR (list-class
// reads override to a longer window; tick-keyed reads carry the refresh tick IN THE KEY so a new tick is
// a new entry — no time-based staleness needed). Retry is off: a denied/missing cap must surface as an
// HONEST denied state immediately (CLAUDE §9), never be retried into a spurious success or a slow spinner.

import { QueryClient } from "@tanstack/react-query";

/** The generous stale window for list-class reads (source picker bundle, datasource list, flow roster) —
 *  they rarely change mid-visit, so a burst of consumers collapses to one fetch and re-reads only after
 *  this window (or an explicit invalidate on workspace switch / editor open where a fresh list matters). */
export const LIST_STALE_MS = 30_000;

/** Mint the per-visit dashboard client. Called once by the provider (via `useState` initialiser) so the
 *  client is stable across the visit and a fresh one is created on the next mount. */
export function makeDashboardQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        // A read either resolves or honestly denies — never retry it into a fabricated success (§9).
        retry: false,
        // No window refocus refetch: the refresh TICK (in the key) is the freshness signal, not focus.
        refetchOnWindowFocus: false,
        // Floor stale window; tick-keyed reads are effectively "fresh until the next tick" via the key.
        staleTime: 0,
        gcTime: LIST_STALE_MS,
      },
    },
  });
}
