// The Data-page hook — data + state for the admin DB browser (data-console scope). Lists tables with
// counts, pages a selected table's raw rows (id-cursor), and reads a bounded relation graph for the
// react-flow view. READ-ONLY: there is no mutation here (the raw grid never writes — edits go through
// the domain verbs). One hook per file (FILE-LAYOUT). Everything runs against the real gateway, and
// every call is admin-gated server-side.

import { useCallback, useEffect, useState } from "react";

import { listTables, readGraph, scanTable } from "@/lib/data/data.api";
import type { Graph, Row, TableCount } from "@/lib/data/data.types";

export interface DataState {
  tables: TableCount[];
  selected: string | null;
  rows: Row[];
  /** The next-page cursor (`null` when fully paged or no table selected). */
  cursor: string | null;
  graph: Graph;
  error: string | null;
  /** Select a table → load its first page of rows (resets paging). */
  select: (table: string) => Promise<void>;
  /** Append the next page of rows for the selected table (cursor paging). */
  more: () => Promise<void>;
  /** Load the relation graph seeded from the selected table, or expand a single record `id`. */
  loadGraph: (id?: string) => Promise<void>;
}

const EMPTY_GRAPH: Graph = { nodes: [], edges: [] };

/** Drive the Data page for the session workspace. */
export function useData(): DataState {
  const [tables, setTables] = useState<TableCount[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [rows, setRows] = useState<Row[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [graph, setGraph] = useState<Graph>(EMPTY_GRAPH);
  const [error, setError] = useState<string | null>(null);

  const fail = (e: unknown) => setError(e instanceof Error ? e.message : String(e));

  // The table picker.
  useEffect(() => {
    listTables().then(setTables).catch(fail);
  }, []);

  const select = useCallback(async (table: string) => {
    setSelected(table);
    setGraph(EMPTY_GRAPH);
    try {
      const page = await scanTable(table);
      setRows(page.rows);
      setCursor(page.next);
      setError(null);
    } catch (e) {
      fail(e);
    }
  }, []);

  const more = useCallback(async () => {
    if (!selected || !cursor) return;
    try {
      const page = await scanTable(selected, undefined, cursor);
      setRows((prev) => [...prev, ...page.rows]);
      setCursor(page.next);
    } catch (e) {
      fail(e);
    }
  }, [selected, cursor]);

  const loadGraph = useCallback(
    async (id?: string) => {
      try {
        // Expanding a node passes its id; the initial graph seeds from the selected table.
        const g = id ? await readGraph(undefined, id) : await readGraph(selected ?? undefined);
        // Merge expand results into the existing graph (dedupe nodes by id).
        setGraph((prev) => mergeGraph(prev, g, !!id));
        setError(null);
      } catch (e) {
        fail(e);
      }
    },
    [selected],
  );

  return { tables, selected, rows, cursor, graph, error, select, more, loadGraph };
}

/** Merge a freshly-read graph slice into the current one. A fresh seed (`!expand`) replaces; an
 *  expand merges (dedupe nodes by id, keep all edges). */
function mergeGraph(prev: Graph, next: Graph, expand: boolean): Graph {
  if (!expand) return next;
  const nodes = new Map(prev.nodes.map((n) => [n.id, n]));
  for (const n of next.nodes) nodes.set(n.id, n);
  return { nodes: [...nodes.values()], edges: [...prev.edges, ...next.edges] };
}
