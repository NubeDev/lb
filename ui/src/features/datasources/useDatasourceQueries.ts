// The datasource-queries hook — the per-source saved-query roster for the Datasource detail page
// (query scope). The page authors raw SQL against one federation datasource; this hook is the one
// place that list/save/remove the resulting `query:{ws}:{id}` records meets the `@/lib/queries`
// client. The list is filtered CLIENT-SIDE to `target === "datasource:<name>"` (the host's
// `query.list` returns the whole workspace's roster; the filter is a pure projection, no second
// call). `lang` defaults to `"raw"` on save; the workbench's PRQL Code mode passes `lang:"prql"`
// so the record compiles to the source's dialect at run (query scope). One hook per file
// (FILE-LAYOUT). No fake/demo data — every call rides the real `invoke` seam to the host bridge.

import { useCallback, useEffect, useState } from "react";

import {
  deleteQuery,
  getQuery,
  listQueries,
  saveQuery,
  type QuerySummary,
  type SavedQuery,
} from "@/lib/queries";

/** The target string for a saved query against this datasource — `datasource:<name>`. The host's
 *  `query.run` resolves this to a registered `datasource:{ws}:{name}` in the caller's workspace. */
export function datasourceTarget(source: string): string {
  return `datasource:${source}`;
}

export interface DatasourceQueries {
  /** Saved queries targeting THIS datasource (filtered from the workspace roster). */
  queries: QuerySummary[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  /** Resolve one saved query to its full record (text included) for loading into the editor. */
  load: (id: string) => Promise<SavedQuery>;
  /** Save the current text as a `query:{ws}:{id}` record (target:datasource:<source>; `lang`
   *  defaults to `raw`, the PRQL Code mode passes `"prql"`). Returns the saved id. Idempotent
   *  UPSERT on `id` — saving the same id overwrites in place. */
  save: (args: {
    id: string;
    name?: string;
    description?: string;
    sql: string;
    lang?: "raw" | "prql";
  }) => Promise<string>;
  /** Soft-delete a saved query (idempotent tombstone). */
  remove: (id: string) => Promise<void>;
}

export function useDatasourceQueries(source: string): DatasourceQueries {
  const [queries, setQueries] = useState<QuerySummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const target = datasourceTarget(source);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const all = await listQueries();
      // Filter to THIS datasource — a workspace's roster includes platform queries and every other
      // datasource's; the detail page only wants this source's.
      setQueries(all.filter((q) => q.target === target));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [target]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const load = useCallback(async (id: string): Promise<SavedQuery> => getQuery(id), []);

  const save = useCallback(
    async (args: {
      id: string;
      name?: string;
      description?: string;
      sql: string;
      lang?: "raw" | "prql";
    }): Promise<string> => {
      const res = await saveQuery({
        id: args.id,
        name: args.name,
        description: args.description,
        lang: args.lang ?? "raw",
        text: args.sql,
        target,
        params: [],
      });
      await refresh();
      return res.id;
    },
    [refresh, target],
  );

  const remove = useCallback(
    async (id: string) => {
      await deleteQuery(id);
      await refresh();
    },
    [refresh],
  );

  return { queries, loading, error, refresh, load, save, remove };
}
