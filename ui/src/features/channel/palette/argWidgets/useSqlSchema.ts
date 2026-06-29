// Schema autocomplete source for the SQL arg widget (channels-command-palette scope). Tables +
// columns come from the SAME native `federation.schema` discovery the datasources page uses
// (`useDatasourceQuery`) — NOT catalog SQL: the federation engine only registers the tables a query
// references, so an `information_schema` SELECT is unplannable. Dialect knowledge lives backend-side.
// The result is cached PER SOURCE (scope "Schema autocomplete cost": don't re-discover on every
// keystroke). One hook per file (FILE-LAYOUT) — data only.

import { useCallback, useEffect, useRef, useState } from "react";

import { describeTable, discoverTables } from "@/lib/datasources";

/** The discovered schema for one source: table names, and columns per table (lazily filled). */
export interface SqlSchema {
  tables: string[];
  columns: Record<string, string[]>;
  loading: boolean;
}

/** Discover + cache the schema for `source` via the native `federation.schema` verb (the backend owns
 *  per-kind catalog access). Tables load when the widget opens; a table's columns load the first time
 *  it is referenced. Best-effort: a discovery failure leaves the schema empty (autocomplete just
 *  offers nothing), never throws into the UI. */
export function useSqlSchema(source: string | null): SqlSchema & { ensureColumns: (t: string) => void } {
  const [schema, setSchema] = useState<SqlSchema>({ tables: [], columns: {}, loading: false });
  const cache = useRef<Map<string, SqlSchema>>(new Map());

  useEffect(() => {
    if (!source) {
      setSchema({ tables: [], columns: {}, loading: false });
      return;
    }
    const cached = cache.current.get(source);
    if (cached) {
      setSchema(cached);
      return;
    }
    let live = true;
    setSchema({ tables: [], columns: {}, loading: true });
    void discoverTables(source)
      .then((ts) => {
        if (!live) return;
        const tables = ts.map((t) => t.name).filter(Boolean);
        const next: SqlSchema = { tables, columns: {}, loading: false };
        cache.current.set(source, next);
        setSchema(next);
      })
      .catch(() => {
        if (live) setSchema({ tables: [], columns: {}, loading: false });
      });
    return () => {
      live = false;
    };
  }, [source]);

  const ensureColumns = useCallback(
    (table: string) => {
      if (!source) return;
      const current = cache.current.get(source);
      if (!current || current.columns[table]) return;
      void describeTable(source, table)
        .then((cols) => {
          const names = cols.map((c) => c.name).filter(Boolean);
          const updated: SqlSchema = { ...current, columns: { ...current.columns, [table]: names } };
          cache.current.set(source, updated);
          setSchema(updated);
        })
        .catch(() => undefined);
    },
    [source],
  );

  return { ...schema, ensureColumns };
}
