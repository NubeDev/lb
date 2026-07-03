// The SHELL ADAPTER for the reusable source picker (source-picker-package-scope.md), now on the dashboard
// READ CACHE (dashboard-query-cache-scope). It is the ONE place `@/lib/*` (the shell's gateway/Tauri API
// clients) meets `@nube/source-picker`: it builds the package's injected `SourceLoaders` from the shipped
// clients and assembles the bundle via the package's PURE `loadSourcePicker` — but routed through
// react-query so the page-level and editor instances SHARE ONE `["source-picker", ws]` cache entry (one
// bundle fetch per workspace, not two). The package stays framework-light (the scope's recommended answer
// to "how much of the package moves"); only this shell adapter adopts react-query.
//
// `datasource.list` is routed through the SHARED `["datasource.list", ws]` query (via `fetchDatasourceList`)
// so the bundle and the Query-tab dropdown collapse to ONE `datasource.list` call.

import { useQuery } from "@tanstack/react-query";
import { useQueryClient } from "@tanstack/react-query";
import {
  loadSourcePicker,
  type SourceEntry,
  type SourceLoaders,
} from "@nube/source-picker";

import { listSeries } from "@/lib/ingest/ingest.api";
import { listExtensions, type ExtRow } from "@/lib/ext/ext.api";
import { listFlows, getFlow, listFlowNodes } from "@/lib/flows/flows.api";
import { sourcePickerKey } from "../cache/queryKeys";
import { fetchDatasourceList } from "../cache/datasourceListQuery";
import { LIST_STALE_MS } from "../cache/dashboardQueryClient";
import type { QueryClient } from "@tanstack/react-query";

/** The shell's picker data — same shape the package returns, but `installed` typed as the shell's fuller
 *  `ExtRow` (what `WidgetView`/`ExtWidget` consume). */
export interface SourcePickerData {
  entries: SourceEntry[];
  installed: ExtRow[];
  loading: boolean;
}

/** Build the shell's read seam. `listDatasources` routes through the SHARED datasource-list cache so the
 *  bundle and the Query-tab dropdown share one `datasource.list` call. */
function shellLoaders(client: QueryClient, ws: string): SourceLoaders {
  return {
    listSeries: () => listSeries(),
    listExtensions: () => listExtensions(),
    listFlows: () => listFlows(),
    getFlow: (id) => getFlow(id),
    listFlowNodes: () => listFlowNodes(),
    listDatasources: () => fetchDatasourceList(client, ws),
  };
}

/** Load + assemble the source picker for the shell, through the read cache. `ws` keys the shared entry —
 *  the page-level and editor instances read the SAME cached bundle (one fetch per workspace). */
export function useSourcePicker(ws: string): SourcePickerData {
  const client = useQueryClient();
  const query = useQuery({
    queryKey: sourcePickerKey(ws),
    queryFn: () => loadSourcePicker(shellLoaders(client, ws)),
    staleTime: LIST_STALE_MS,
  });
  // `installed` is the real shell `ExtRow` at runtime (from `listExtensions`); the package types it as its
  // structural subset. Re-assert the shell type for consumers that need the fuller row.
  return {
    entries: query.data?.entries ?? [],
    installed: (query.data?.installed ?? []) as ExtRow[],
    loading: query.isLoading,
  };
}
