// The datasource roster for the Query tab's datasource dropdown (viz datasource-binding scope, Phase 3).
// It loads the workspace's registered FEDERATION sources from the SHIPPED ws-walled `datasource.list`
// verb (`listDatasources`) and prepends the two BUILT-IN datasources every workspace always has: native
// SurrealDB and Series. A denied `datasource.list` is an HONEST empty federation list (the built-ins
// still show), never a fabricated roster (CLAUDE §9). One hook per file (FILE-LAYOUT).

import { useEffect, useState } from "react";

import { listDatasources } from "@/lib/datasources";

/** One option in the datasource dropdown — a built-in or a registered federation source. */
export interface DatasourceOption {
  /** The `DataSourceRef.type` this option binds onto the target. */
  type: "surreal" | "series" | "federation";
  /** The display label. */
  label: string;
  /** The federation source NAME (federation only) — also the `uid` tail `datasource:{ws}:{name}`. */
  name?: string;
}

/** The two built-ins every workspace has, regardless of `datasource.list` — native store + series. */
const BUILTINS: DatasourceOption[] = [
  { type: "surreal", label: "SurrealDB (native)" },
  { type: "series", label: "Series" },
];

/** Load the datasource options for `ws`: the two built-ins + each registered federation source. A
 *  denied/failed list collapses to just the built-ins (honest), never invented entries. */
export function useDatasourceList(ws: string): { options: DatasourceOption[]; loading: boolean } {
  const [federation, setFederation] = useState<DatasourceOption[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    listDatasources()
      .then((rows) => {
        if (cancelled) return;
        setFederation(rows.map((d) => ({ type: "federation" as const, label: `${d.name} (${d.kind})`, name: d.name })));
        setLoading(false);
      })
      .catch(() => {
        if (cancelled) return;
        setFederation([]); // denied → built-ins only, never a fabricated roster
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [ws]);

  return { options: [...BUILTINS, ...federation], loading };
}

/** Build the `DataSourceRef` for a chosen option (`uid` set for federation: `datasource:{ws}:{name}`). */
export function refForOption(opt: DatasourceOption, ws: string): { type: string; uid?: string } {
  if (opt.type === "federation" && opt.name) return { type: "federation", uid: `datasource:${ws}:${opt.name}` };
  return { type: opt.type };
}
