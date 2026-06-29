// The datasource query hook (datasources-ux scope) — the one place the detail page assembles table
// discovery, column discovery, and ad-hoc query results from the real `federation.query` verb. No
// fake/demo data — every call goes through the real `invoke` seam to the gateway/host sidecar. The
// generated discovery SELECTs are IDENTIFIER-QUOTED (double quotes, embedded quotes escaped) so a
// picked table name can't break out of the identifier; SELECT-only is also re-validated host-side.
// One hook per file (FILE-LAYOUT).

import { useCallback, useState } from "react";

import { runFederationQuery } from "@/lib/datasources";
import type { DbColumn, DbTable, FederationQueryResult } from "@/lib/datasources";

/** Quote a SQL identifier (double-quoted, with embedded `"` doubled) so a table name can never break
 *  out of the identifier position. Trusted shell code generating the discovery SELECTs — but quoted
 *  regardless, defense in depth (SELECT-only is re-validated host-side too). */
function ident(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

/** The `SELECT` that lists user tables in the source, by kind. Postgres uses `information_schema`
 *  (+ a cheap `reltuples` estimate); sqlite uses `sqlite_master`. */
function listTablesSql(kind: string): string {
  if (kind === "sqlite") {
    return (
      "SELECT name AS name FROM sqlite_master " +
      "WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    );
  }
  return (
    "SELECT t.table_name AS name, COALESCE(c.reltuples::bigint, 0) AS rows " +
    "FROM information_schema.tables t " +
    "LEFT JOIN pg_class c ON c.relname = t.table_name " +
    "WHERE t.table_schema = 'public' AND t.table_type = 'BASE TABLE' " +
    "ORDER BY t.table_name"
  );
}

/** The `SELECT` that describes one table's columns, by kind. */
function describeTableSql(kind: string, table: string): string {
  if (kind === "sqlite") {
    return (
      `SELECT name AS name, type AS data_type, "notnull" = 0 AS nullable ` +
      `FROM pragma_table_info(${ident(table)}) ORDER BY cid`
    );
  }
  return (
    "SELECT column_name AS name, data_type AS data_type, is_nullable = 'YES' AS nullable " +
    `FROM information_schema.columns WHERE table_schema = 'public' AND table_name = ${ident(table)} ` +
    "ORDER BY ordinal_position"
  );
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

/** Map a federation row-set onto `DbTable[]` (tolerant of missing `rows`). */
function toTables(rows: Record<string, unknown>[]): DbTable[] {
  return rows.map((r) => {
    const name = String(r.name ?? "");
    const raw = r.rows;
    return {
      name,
      rows: typeof raw === "number" ? raw : typeof raw === "string" ? Number(raw) : undefined,
    };
  });
}

/** Map a federation row-set onto `DbColumn[]`. */
function toColumns(rows: Record<string, unknown>[]): DbColumn[] {
  return rows.map((r) => ({
    name: String(r.name ?? ""),
    dataType: String(r.data_type ?? ""),
    nullable: Boolean(r.nullable),
  }));
}

/** The hook. `source` is the registered datasource name (workspace-pinned host-side); `kind` selects
 *  the discovery SQL dialect (postgres vs sqlite). */
export function useDatasourceQuery(source: string, kind: string): DatasourceQuery {
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
    await exec(
      listTablesSql(kind),
      (r) => setTables(toTables(r.rows)),
      { keepResult: false },
    );
  }, [exec, kind]);

  const describeTable = useCallback(
    async (table: string) => {
      setColumns(null);
      await exec(
        describeTableSql(kind, table),
        (r) => setColumns(toColumns(r.rows)),
        { keepResult: false },
      );
    },
    [exec, kind],
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
