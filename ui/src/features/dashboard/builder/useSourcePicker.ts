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
import { listRules } from "@/lib/rules/rules.api";
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
    // Saved rules → the Rules group (each ⇒ a `rules.run {rule_id}` read source). `SavedRule` is a
    // structural superset of the package's `RuleSummary` ({id,name}); a workspace without the
    // `mcp:rules.list:call` grant sees `rules_list` reject → an empty group (deny-tolerant).
    listRules: () => listRules(),
  };
}

/** Load + assemble the source picker for the shell, through the read cache. `ws` keys the shared entry —
 *  the page-level and editor instances read the SAME cached bundle (one fetch per workspace).
 *
 *  LAZY by default (`{ enabled: false }`): the query does NOT fire on mount. A caller that wants the
 *  entries immediately passes `{ enabled: true }` (the legacy eager behaviour — `DashboardView`, which
 *  needs `installed` for tile rendering); a caller that only needs them on user interaction (e.g. the
 *  QueryTab's source combobox — opens on focus) keeps the default and flips `enabled` to true when the
 *  user actually picks. This is the "don't fire on page load, only on demand" contract — a restored
 *  builder tab no longer fans out every `*.list` verb on Data Studio mount. */
export function useSourcePicker(
  ws: string,
  opts: { enabled?: boolean } = {},
): SourcePickerData {
  const client = useQueryClient();
  const enabled = opts.enabled ?? false;
  const query = useQuery({
    queryKey: sourcePickerKey(ws),
    queryFn: () => loadSourcePicker(shellLoaders(client, ws)),
    staleTime: LIST_STALE_MS,
    enabled,
  });
  // `installed` is the real shell `ExtRow` at runtime (from `listExtensions`); the package types it as its
  // structural subset. Re-assert the shell type for consumers that need the fuller row.
  return {
    entries: query.data?.entries ?? [],
    installed: (query.data?.installed ?? []) as ExtRow[],
    // When the query isn't enabled, surface `loading: false` (not the react-query `isLoading: true`
    // for an unmounted query) so the UI doesn't show a perpetual spinner for a deliberately-deferred
    // load. Once enabled, the real `isLoading` takes over.
    loading: enabled ? query.isLoading : false,
  };
}
