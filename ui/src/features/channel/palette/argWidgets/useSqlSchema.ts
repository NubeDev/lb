// Schema autocomplete source for the SQL arg widget (channels-command-palette scope). Tables +
// columns come from the SAME discovery SELECTs the datasources page uses (`useDatasourceQuery`),
// run through the real `federation.query` bridge — no new verb, no fake. The result is cached PER
// SOURCE (scope "Schema autocomplete cost": don't re-discover on every keystroke). One hook per
// file (FILE-LAYOUT) — data only.

import { useCallback, useEffect, useRef, useState } from "react";

import { runFederationQuery } from "@/lib/datasources";

/** The discovered schema for one source: table names, and columns per table (lazily filled). */
export interface SqlSchema {
  tables: string[];
  columns: Record<string, string[]>;
  loading: boolean;
}

// The discovery SELECTs — sqlite + postgres, mirroring useDatasourceQuery. Quoted identifiers so a
// table name can't break out (defense in depth; SELECT-only is re-validated host-side too).
const ident = (n: string) => `"${n.replace(/"/g, '""')}"`;

function listTablesSql(kind: string): string {
  if (kind === "sqlite") {
    return "SELECT name AS name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name";
  }
  return (
    "SELECT table_name AS name FROM information_schema.tables " +
    "WHERE table_schema = 'public' AND table_type = 'BASE TABLE' ORDER BY table_name"
  );
}

function describeTableSql(kind: string, table: string): string {
  if (kind === "sqlite") {
    return `SELECT name AS name FROM pragma_table_info(${ident(table)}) ORDER BY cid`;
  }
  return (
    "SELECT column_name AS name FROM information_schema.columns " +
    `WHERE table_schema = 'public' AND table_name = ${ident(table)} ORDER BY ordinal_position`
  );
}

/** Discover + cache the schema for `source` (kind selects the SQL dialect). Tables load when the
 *  widget opens; a table's columns load the first time it is referenced. Best-effort: a discovery
 *  failure leaves the schema empty (autocomplete just offers nothing), never throws into the UI. */
export function useSqlSchema(source: string | null, kind: string): SqlSchema & { ensureColumns: (t: string) => void } {
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
    void runFederationQuery(source, listTablesSql(kind))
      .then((r) => {
        if (!live) return;
        const tables = r.rows.map((row) => String(row.name ?? "")).filter(Boolean);
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
  }, [source, kind]);

  const ensureColumns = useCallback(
    (table: string) => {
      if (!source) return;
      const current = cache.current.get(source);
      if (!current || current.columns[table]) return;
      void runFederationQuery(source, describeTableSql(kind, table))
        .then((r) => {
          const cols = r.rows.map((row) => String(row.name ?? "")).filter(Boolean);
          const updated: SqlSchema = { ...current, columns: { ...current.columns, [table]: cols } };
          cache.current.set(source, updated);
          setSchema(updated);
        })
        .catch(() => undefined);
    },
    [source, kind],
  );

  return { ...schema, ensureColumns };
}
