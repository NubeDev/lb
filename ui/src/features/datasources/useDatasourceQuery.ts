// The datasource query hook (datasources-ux scope) — the one place the detail page assembles table
// discovery, column discovery, and ad-hoc query results. Table/column DISCOVERY goes through the
// native `federation.schema` verb (NOT catalog SQL): the federation engine only registers the tables
// a query references as DataFusion providers, so an `information_schema`/`pg_class` SELECT is
// unplannable ("table not found"). Dialect knowledge lives backend-side in the sidecar — the UI must
// not hand-write catalog SQL. Ad-hoc/preview queries still run through `federation.query`. Every call
// goes through the real `invoke` seam to the gateway/host sidecar; no fake/demo data. One hook per
// file (FILE-LAYOUT).

import { useCallback, useState } from "react";

import {
  describeTable as describeTableApi,
  discoverTables as discoverTablesApi,
  runFederationQuery,
} from "@/lib/datasources";
import type { DbColumn, DbTable, FederationQueryResult } from "@/lib/datasources";

/** Quote a SQL identifier (double-quoted, with embedded `"` doubled) so a table name can never break
 *  out of the identifier position in the preview SELECT (SELECT-only is re-validated host-side too). */
function ident(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

/** A bounded `SELECT *` preview of one table — the no-SQL "just show me the rows" affordance. */
function previewTableSql(table: string, limit: number): string {
  return `SELECT * FROM ${ident(table)} LIMIT ${limit}`;
}

export interface DatasourceQuery {
  /** Discovered tables (null before first load / while loading). */
  tables: DbTable[] | null;
  /** Discovered columns for the selected table (null before a table is picked). */
  columns: DbColumn[] | null;
  /** The last query result (table preview or a run-SQL result). */
  result: FederationQueryResult | null;
  /** The SQL that produced `result` (so the editor can echo what the builder generated). */
  lastSql: string | null;
  loading: boolean;
  error: string | null;
  discoverTables: () => Promise<void>;
  describeTable: (table: string) => Promise<void>;
  previewTable: (table: string, limit?: number) => Promise<void>;
  runSql: (sql: string) => Promise<void>;
  reset: () => void;
}

/** The hook. `source` is the registered datasource name (workspace-pinned host-side). Discovery picks
 *  no dialect: the backend `federation.schema` verb owns per-kind catalog access. */
export function useDatasourceQuery(source: string): DatasourceQuery {
  const [tables, setTables] = useState<DbTable[] | null>(null);
  const [columns, setColumns] = useState<DbColumn[] | null>(null);
  const [result, setResult] = useState<FederationQueryResult | null>(null);
  const [lastSql, setLastSql] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const exec = useCallback(
    async (
      sql: string,
      onRows: (r: FederationQueryResult) => void,
      opts: { keepResult?: boolean } = {},
    ) => {
      setLoading(true);
      setError(null);
      try {
        const r = await runFederationQuery(source, sql);
        if (opts.keepResult) {
          setResult(r);
          setLastSql(sql);
        }
        onRows(r);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setLoading(false);
      }
    },
    [source],
  );

  const discoverTables = useCallback(async () => {
    setTables(null);
    setColumns(null);
    setResult(null);
    setLastSql(null);
    setLoading(true);
    setError(null);
    try {
      setTables(await discoverTablesApi(source));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [source]);

  const describeTable = useCallback(
    async (table: string) => {
      setColumns(null);
      setLoading(true);
      setError(null);
      try {
        setColumns(await describeTableApi(source, table));
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setLoading(false);
      }
    },
    [source],
  );

  const previewTable = useCallback(
    async (table: string, limit = 100) => {
      const sql = previewTableSql(table, limit);
      await exec(sql, () => undefined, { keepResult: true });
    },
    [exec],
  );

  const runSql = useCallback(
    async (sql: string) => {
      await exec(sql, () => undefined, { keepResult: true });
    },
    [exec],
  );

  return {
    tables,
    columns,
    result,
    lastSql,
    loading,
    error,
    discoverTables,
    describeTable,
    previewTable,
    runSql,
    reset: () => {
      setTables(null);
      setColumns(null);
      setResult(null);
      setLastSql(null);
      setError(null);
    },
  };
}
