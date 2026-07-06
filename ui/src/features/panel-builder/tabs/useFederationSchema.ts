// The FEDERATION schema loader for the panel-builder Query tab (query-builder-common scope). One
// responsibility: load a registered federation source's tables + the SELECTED table's columns
// through the shipped `federation.schema` verb, projected into the same `Schema` shape the local
// `readSchema()` produces — so `SqlQueryEditor` consumes ONE shape regardless of dialect. The
// editor stays transport-agnostic; this hook is the federation/host side of the contract.
//
// Lazy per-table column fill: tables load once for the source; a table's columns load the first
// time it's referenced (the same pattern `useSqlSchema` already proved for the channels palette).
// `describeTable`'s `{name, dataType, nullable}` row projects onto `SchemaColumn` (`dataType` →
// `type`); `VisualEditor` only reads `.name`, but the projection keeps the shape symmetric.
//
// Honesty contract (rule 9): a deny (no `mcp:federation.query:call`) or load failure or unknown
// source collapses to an EMPTY `tables: []` — the dropdown is empty (the system-catalog deny
// contract), the Code half still works. The host's per-call wall + workspace pinning enforce the
// isolation; nothing client-side can name a cross-tenant source.

import { useEffect, useRef, useState } from "react";

import { describeTable, discoverTables } from "@/lib/datasources";
import type { Schema, SchemaColumn, SchemaTable } from "@/lib/schema";

const EMPTY: Schema = { tables: [] };

/**
 * @param source The registered federation datasource name (`target.args.source`), or `null` when
 *   the editor has no federation target yet.
 * @param table The builder's currently-selected table (drives the lazy column fill). Empty when no
 *   table is picked.
 * @param enabled Whether to fire the discovery verbs at all — the Query tab is lazy (a restored
 *   empty builder tab must not fire `federation.schema` on mount).
 */
export function useFederationSchema(
  source: string | null,
  table: string,
  enabled: boolean,
): Schema {
  const [schema, setSchema] = useState<Schema>(EMPTY);
  // Per-source column cache so picking/re-picking a table never re-fetches.
  const columnsByTable = useRef<Map<string, SchemaColumn[]>>(new Map());
  const tableOrder = useRef<string[]>([]);

  // (1) Load tables when `source` changes (and discovery is enabled).
  useEffect(() => {
    columnsByTable.current = new Map();
    tableOrder.current = [];
    if (!enabled || !source) {
      setSchema(EMPTY);
      return;
    }
    let cancelled = false;
    setSchema(EMPTY);
    discoverTables(source)
      .then((rows) => {
        if (cancelled) return;
        const names = rows.map((r) => r.name).filter(Boolean);
        tableOrder.current = names;
        // Tables appear immediately with empty columns; the table the builder has picked gets its
        // columns filled by effect (2) below.
        setSchema({
          tables: names.map<SchemaTable>((name) => ({
            name,
            columns: columnsByTable.current.get(name) ?? [],
          })),
        });
      })
      .catch(() => {
        if (!cancelled) setSchema(EMPTY);
      });
    return () => {
      cancelled = true;
    };
  }, [source, enabled]);

  // (2) Lazy-fill the SELECTED table's columns. Fires once per (source, table) pair.
  useEffect(() => {
    if (!enabled || !source || !table || columnsByTable.current.has(table)) return;
    let cancelled = false;
    describeTable(source, table)
      .then((cols) => {
        if (cancelled) return;
        const projected: SchemaColumn[] = cols.map((c) => ({ name: c.name, type: c.dataType }));
        columnsByTable.current.set(table, projected);
        setSchema((prev) => ({
          tables: tableOrder.current.map<SchemaTable>((name) => ({
            name,
            columns:
              name === table
                ? projected
                : columnsByTable.current.get(name) ?? prev.tables.find((t) => t.name === name)?.columns ?? [],
          })),
        }));
      })
      .catch(() => {
        // A describe failure leaves the table column-less (honest) but does not drop the table.
        if (!cancelled) columnsByTable.current.set(table, []);
      });
    return () => {
      cancelled = true;
    };
  }, [source, table, enabled]);

  return schema;
}
