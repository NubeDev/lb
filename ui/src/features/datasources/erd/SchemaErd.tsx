// The schema-ERD canvas host (datasources-ux ERD scope) — the "Diagram" view of the Discovery tab. It
// renders the selected datasource's tables + columns as a React Flow graph: one node per table, dashed
// edges for naming-convention-inferred relationships. Lazy-loaded by DatasourceDetail so `@xyflow/react`
// only ships on the Discovery → Diagram tab (mirror of data/DataGraph). Reuses the EXISTING
// `federation.schema` verb via the shared `describeTable` api to fetch each table's columns — NO new
// MCP verb/cap/backend (rule 9 — no fakes; the sidecar's real catalog is read). Clicking a node selects
// that table in the SAME Discovery state the rail writes to (list ⇄ diagram share selection). Node
// positions are ephemeral (v1 — no persistence). One responsibility, one file (FILE-LAYOUT).

import { useEffect, useMemo, useState } from "react";
import {
  Background,
  Controls,
  Panel,
  ReactFlow,
  type Edge,
  type Node,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { Loader2, Network } from "lucide-react";

import { describeTable } from "@/lib/datasources";
import type { DbColumn, DbTable } from "@/lib/datasources";
import { layoutErd } from "./erdLayout";
import { schemaToFlow, type SchemaTableNodeData } from "./schemaToFlow";
import { SchemaTableNode } from "./SchemaTableNode";

const nodeTypes = { schemaTable: SchemaTableNode };

interface Props {
  /** Registered datasource name — the `federation.schema` `source` arg (workspace-pinned host-side). */
  source: string;
  /** The discovered tables (the rail's list). Null before first load. */
  tables: DbTable[] | null;
  /** The currently selected table (shared with the rail) — its node renders highlighted. */
  selectedTable: string | null;
  /** Select a table (shared with the rail) — fired on node click. */
  onSelect: (table: string) => void;
}

/** Fetch every discovered table's columns ONCE per table set, through the real `federation.schema` verb
 *  (the same loader the COLUMNS panel uses). Per-table failures degrade to an empty column list — the
 *  node still renders; an honest error surfaces only when the host is unreachable for EVERY table. */
function useSchemaColumns(source: string, tables: DbTable[] | null): {
  columnsByTable: Record<string, DbColumn[]>;
  loading: boolean;
  error: string | null;
} {
  const [columnsByTable, setColumnsByTable] = useState<Record<string, DbColumn[]>>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!tables || tables.length === 0) {
      setColumnsByTable({});
      setError(null);
      return;
    }
    // Captured per-run so a stale fetch (tables changed before this resolved) is discarded — only the
    // latest effect's `alive` is true after a change (its predecessor's cleanup flipped it).
    let alive = true;
    setLoading(true);
    setError(null);
    Promise.allSettled(tables.map((t) => describeTable(source, t.name)))
      .then((results) => {
        if (!alive) return;
        const next: Record<string, DbColumn[]> = {};
        let rejections = 0;
        let firstErr = "";
        results.forEach((r, i) => {
          const tName = tables[i].name;
          if (r.status === "fulfilled") {
            next[tName] = r.value;
          } else {
            rejections++;
            firstErr ||= r.reason instanceof Error ? r.reason.message : String(r.reason);
            next[tName] = [];
          }
        });
        setColumnsByTable(next);
        // Only an across-the-board failure is surfaced as an error — partial failures still render.
        setError(rejections === tables.length ? firstErr : null);
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [source, tables]);

  return { columnsByTable, loading, error };
}

export default function SchemaErd({ source, tables, selectedTable, onSelect }: Props) {
  const { columnsByTable, loading, error } = useSchemaColumns(source, tables);

  const { nodes, edges, relations } = useMemo(() => {
    if (!tables || tables.length === 0) {
      return { nodes: [] as Node<SchemaTableNodeData>[], edges: [] as Edge[], relations: [] };
    }
    const erdTables = tables.map((t) => ({
      name: t.name,
      columns: columnsByTable[t.name] ?? [],
    }));
    const flow = schemaToFlow(erdTables);
    const positioned = layoutErd(flow.nodes, flow.edges);
    // Mark the shared-selection node so the diagram + rail agree on which table is active.
    const marked = positioned.map((n) => ({ ...n, selected: n.id === selectedTable }));
    return { nodes: marked, edges: flow.edges, relations: flow.relations };
  }, [tables, columnsByTable, selectedTable]);

  if (tables !== null && tables.length === 0) {
    return (
      <div className="flex h-full items-center justify-center bg-bg p-8 text-center">
        <div className="max-w-sm">
          <div className="mx-auto flex h-10 w-10 items-center justify-center rounded-md border border-border bg-panel">
            <Network size={18} className="text-accent" />
          </div>
          <h2 className="mt-3 text-sm font-medium">No tables to diagram</h2>
          <p className="mt-1 text-sm text-muted">
            Discovery returned no tables for this datasource.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col" data-testid="schema-erd">
      <div className="flex min-h-[3.25rem] items-center justify-between gap-3 border-b border-border bg-bg px-3 py-2">
        <div className="min-w-0">
          <div className="text-sm font-medium">Schema diagram</div>
          <div className="truncate text-xs text-muted">
            {nodes.length} table{nodes.length === 1 ? "" : "s"} · {relations.length} inferred edge
            {relations.length === 1 ? "" : "s"}
          </div>
        </div>
        <div className="hidden rounded-md border border-border bg-panel px-2 py-1 text-xs text-muted sm:block">
          Click a table to inspect it
        </div>
      </div>

      {error && (
        <div
          role="alert"
          className="border-b border-destructive/30 bg-destructive/10 px-3 py-1.5 text-xs text-destructive"
        >
          {error}
        </div>
      )}

      <div className="relative min-h-0 flex-1 bg-bg">
        {loading && (
          <div className="absolute left-1/2 top-3 z-10 -translate-x-1/2 rounded-md border border-border bg-panel px-2 py-1 text-xs text-muted shadow-sm">
            <Loader2 size={12} className="mr-1 inline animate-spin" />
            reading columns…
          </div>
        )}
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodeClick={(_e, node) => onSelect(node.id)}
          fitView
          proOptions={{ hideAttribution: true }}
        >
          <Background color="hsl(var(--border))" gap={40} size={1} />
          <Controls
            showInteractive={false}
            className="overflow-hidden rounded-md border border-border bg-panel text-fg"
          />
          {relations.length > 0 && (
            <Panel position="top-left" className="!m-2">
              <div className="rounded-md border border-border bg-panel/95 px-2 py-1.5 text-[11px] text-muted shadow-sm">
                <span className="mr-1.5 inline-block h-0 w-4 align-middle border-t-2 border-dashed border-muted" />
                edges inferred by naming convention
              </div>
            </Panel>
          )}
        </ReactFlow>
      </div>
    </div>
  );
}
