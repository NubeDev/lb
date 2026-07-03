// The shared `datasource.list` read (dashboard-query-cache-scope). ONE definition of "how to fetch the
// workspace's federation datasources", so every consumer collapses to a single cache entry: the Query-tab
// dropdown (`useDatasourceList`) AND the source-picker bundle (whose `listDatasources` loader routes here
// via `fetchQuery`) read the same `["datasource.list", ws]` key → `datasource.list` fires ONCE per ws, not
// the 2–3× the scope flagged. One responsibility: the datasource-list query descriptor.

import type { QueryClient } from "@tanstack/react-query";

import { listDatasources } from "@/lib/datasources";
import type { DatasourceSummary } from "@/lib/datasources";
import { datasourceListKey } from "./queryKeys";
import { LIST_STALE_MS } from "./dashboardQueryClient";

/** The query options for `datasource.list` in workspace `ws`. A list-class read (generous stale window):
 *  it rarely changes mid-visit, so a burst of consumers collapses to one fetch. */
export function datasourceListQueryOptions(ws: string) {
  return {
    queryKey: datasourceListKey(ws),
    queryFn: (): Promise<DatasourceSummary[]> => listDatasources(),
    staleTime: LIST_STALE_MS,
  };
}

/** Fetch (or read warm) the datasource list through the shared cache — used by the source-picker adapter's
 *  `listDatasources` loader so the bundle and the dropdown share the one call. */
export function fetchDatasourceList(client: QueryClient, ws: string): Promise<DatasourceSummary[]> {
  return client.fetchQuery(datasourceListQueryOptions(ws));
}
