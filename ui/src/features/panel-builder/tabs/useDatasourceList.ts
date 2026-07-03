// The datasource roster for the Query tab's datasource dropdown (viz datasource-binding scope, Phase 3).
// It loads the workspace's registered FEDERATION sources from the SHIPPED ws-walled `datasource.list`
// verb (`listDatasources`) and prepends the two BUILT-IN datasources every workspace always has: native
// SurrealDB and Series. A denied `datasource.list` is an HONEST empty federation list (the built-ins
// still show), never a fabricated roster (CLAUDE §9). One hook per file (FILE-LAYOUT).

import { useQuery } from "@tanstack/react-query";

import { datasourceListQueryOptions } from "@/features/dashboard/cache/datasourceListQuery";

/** One option in the datasource dropdown — a built-in or a registered federation source. */
export interface DatasourceOption {
  /** The `DataSourceRef.type` this option binds onto the target. */
  type: "surreal" | "series" | "federation" | "flows";
  /** The display label. */
  label: string;
  /** The federation source NAME (federation only) — also the `uid` tail `datasource:{ws}:{name}`. */
  name?: string;
}

/** The built-ins every workspace has, regardless of `datasource.list` — native store + series + the
 *  Flows binding (flow-dashboard-binding-ux-scope: pick a flow node + port → a control or read view). */
const BUILTINS: DatasourceOption[] = [
  { type: "surreal", label: "SurrealDB (native)" },
  { type: "series", label: "Series" },
  { type: "flows", label: "Flows (node ports)" },
];

/** Load the datasource options for `ws`: the two built-ins + each registered federation source. A
 *  denied/failed list collapses to just the built-ins (honest), never invented entries. Routes through the
 *  SHARED `["datasource.list", ws]` cache (dashboard-query-cache-scope), so this and the source-picker
 *  bundle collapse to ONE `datasource.list` call per workspace. */
export function useDatasourceList(ws: string): { options: DatasourceOption[]; loading: boolean } {
  // `retry:false` in the client default means a denied list rejects → `data` stays undefined → built-ins
  // only (honest, never a fabricated roster).
  const { data, isLoading } = useQuery(datasourceListQueryOptions(ws));
  const federation: DatasourceOption[] = (data ?? []).map((d) => ({
    type: "federation" as const,
    label: `${d.name} (${d.kind})`,
    name: d.name,
  }));
  return { options: [...BUILTINS, ...federation], loading: isLoading };
}

/** Build the `DataSourceRef` for a chosen option (`uid` set for federation: `datasource:{ws}:{name}`). */
export function refForOption(opt: DatasourceOption, ws: string): { type: string; uid?: string } {
  if (opt.type === "federation" && opt.name) return { type: "federation", uid: `datasource:${ws}:${opt.name}` };
  return { type: opt.type };
}
