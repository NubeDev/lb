// The schema-designer canvas (schema-designer scope) — the React Flow host for an editable schema.
// Renders the table nodes + FK edges, handles add-table / auto-layout / import-from-source / the
// column-handle→column-handle FK drag, and mirrors every mutation back into the record state. The
// owning page (`SchemaDesignerPage`) owns load/save; this component owns the live graph. shadcn-
// first. One responsibility, one file (FILE-LAYOUT).
//
// **Provenance:** the canvas skeleton (ReactFlow + Background + Controls + nodeTypes + onConnect)
// follows the shipped ERD (`erd/SchemaErd.tsx`) and the flows canvas (`FlowCanvas.tsx`). The
// editable node + the import/save affordances are original. tabularis (Apache-2.0) inspired the
// node shape; ChartDB (AGPL) is UX reference only — no code copied (scope license hygiene).

import { useCallback, useEffect, useMemo } from "react";
import {
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
  Background,
  Controls,
  MarkerType,
  Panel,
  ReactFlow,
  type Connection,
  type Edge,
  type EdgeChange,
  type NodeChange,
  type Node,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { LayoutGrid, Loader2, Plus, Upload } from "lucide-react";

import { Button } from "@/components/ui/button";
import { describeTable } from "@/lib/datasources";
import type { DbSchemaRecord } from "@/lib/datasources";
import {
  EditableTableNode,
  SchemaDesignerNodeContext,
  SCHEMA_TABLE_NODE_TYPE,
} from "./EditableTableNode";
import type { EditableTableNodeData } from "./recordFlow";
import { layoutWithDagre } from "./designerLayout";
import { recordToFlow } from "./recordFlow";

interface Props {
  record: DbSchemaRecord;
  onChange: (next: DbSchemaRecord) => void;
  /** When non-null, "Import from source" loads this datasource's live tables into the canvas. */
  importSource: string | null;
  /** Set true while import-from-source is reading the catalog. */
  importing: boolean;
  /** Callback after an import completes (the page clears its importing state). */
  onImportDone: () => void;
}

/** One FK edge's default styling (solid accent line with an arrowhead). */
const FK_EDGE_DEFAULTS = {
  type: "smoothstep" as const,
  markerEnd: { type: MarkerType.ArrowClosed, color: "hsl(var(--accent))" },
  style: { stroke: "hsl(var(--accent))", strokeWidth: 1.5 },
};

/**
 * The schema-designer canvas. Mutations (drag, add column, rename, FK drag) are mirrored into the
 * record via `onChange`. The canvas is a CONTROLLED React Flow — it owns no state beyond the graph
 * projection of the record + the transient node/edge change handlers.
 */
export function SchemaDesignerCanvas({
  record,
  onChange,
  importSource,
  importing,
  onImportDone,
}: Props) {
  const { nodes: initialNodes, edges: initialEdges } = useMemo(
    () => recordToFlow(record),
    // Re-project only when the record NAME changes (a different schema loaded). Live edits mutate
    // the graph below; re-projecting on every keystroke would fight the author's focus + cursor.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [record.name],
  );

  const nodeTypes = useMemo(
    () => ({ [SCHEMA_TABLE_NODE_TYPE]: EditableTableNode as unknown as typeof EditableTableNode }),
    [],
  );

  /** The node-edit callback the canvas provides via Context. A node reports an inline edit (rename,
   *  type change, add/remove column, toggle PK/nullable) → the canvas merges it into the record. */
  const onNodeUpdate = useCallback(
    (id: string, next: EditableTableNodeData) => {
      const tables = record.tables.map((t) =>
        t.name === id
          ? {
              name: next.name,
              pk: next.columns.filter((c) => c.pk).map((c) => c.name),
              columns: next.columns.map((c) => ({
                name: c.name,
                type: c.type,
                nullable: c.nullable,
              })),
            }
          : t,
      );
      // If the table was renamed, follow the new name so the side panel + edges stay bound.
      const renamedTo = next.name;
      onChange({ ...record, tables, layout: renameLayoutKey(record.layout, id, renamedTo) });
    },
    [record, onChange],
  );

  const onNodesChange = useCallback(
    (changes: NodeChange[]) => {
      const next = applyNodeChanges(changes, initialNodes) as Node<EditableTableNodeData>[];
      onChange(mergeNodes(record, next));
    },
    [initialNodes, record, onChange],
  );
  const onEdgesChange = useCallback(
    (changes: EdgeChange[]) => {
      const next = applyEdgeChanges(changes, initialEdges);
      onChange(mergeEdges(record, next));
    },
    [initialEdges, record, onChange],
  );
  const onConnect = useCallback(
    (conn: Connection) => {
      // A column→column drag creates a DECLARED FK edge. source = the from-table's column handle;
      // target = the to-table's column handle. The edge's data carries `declared: true` so
      // `flowToRecord` projects it into the record's `fks`.
      if (!conn.sourceHandle || !conn.targetHandle || !conn.source || !conn.target) return;
      const edge: Edge = {
        ...FK_EDGE_DEFAULTS,
        id: `fk:${conn.source}:${conn.sourceHandle}->${conn.target}:${conn.targetHandle}:${Date.now()}`,
        source: conn.source,
        sourceHandle: conn.sourceHandle,
        target: conn.target,
        targetHandle: conn.targetHandle,
        label: conn.sourceHandle,
        labelBgPadding: [4, 2],
        labelBgBorderRadius: 3,
        labelBgStyle: { fill: "hsl(var(--bg))", fillOpacity: 0.9 },
        labelStyle: {
          fill: "hsl(var(--muted))",
          fontSize: 10,
          fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
        },
        data: { declared: true, name: "" },
      };
      const next = addEdge(edge, initialEdges);
      onChange(mergeEdges(record, next));
    },
    [initialEdges, record, onChange],
  );

  // The import-from-source path: read the live catalog's tables + columns and merge as new nodes.
  useEffect(() => {
    if (!importSource || importing) return;
    let alive = true;
    (async () => {
      try {
        const tables = await import("../../../lib/datasources").then((m) => m.discoverTables(importSource));
        if (!alive) return;
        const columnsByTable = await Promise.all(
          tables.map(async (t) => [t.name, await describeTable(importSource, t.name)] as const),
        );
        if (!alive) return;
        const importedTables = tables.map((t) => {
          const cols = columnsByTable.find(([n]) => n === t.name)?.[1] ?? [];
          return {
            name: t.name,
            pk: [] as string[],
            columns: cols.map((c) => ({
              name: c.name,
              type: guessNeutralType(c.dataType),
              nullable: c.nullable,
            })),
          };
        });
        const merged: DbSchemaRecord = {
          ...record,
          tables: [...record.tables, ...importedTables.filter((it) => !record.tables.some((rt) => rt.name === it.name))],
        };
        onChange(merged);
      } finally {
        if (alive) onImportDone();
      }
    })();
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [importSource, importing]);

  const addTable = () => {
    const n = record.tables.length + 1;
    const name = `table_${n}`;
    const merged: DbSchemaRecord = {
      ...record,
      tables: [
        ...record.tables,
        { name, pk: [], columns: [{ name: "id", type: "integer", nullable: false }] },
      ],
      layout: { ...record.layout, [name]: { x: 40 + record.tables.length * 40, y: 40 } },
    };
    onChange(merged);
  };

  const autoLayout = () => {
    const { nodes } = recordToFlow(record);
    const laid = layoutWithDagre(nodes, recordToFlow(record).edges);
    const layout: Record<string, { x: number; y: number }> = { ...record.layout };
    for (const n of laid) {
      layout[n.data.name] = { x: n.position.x, y: n.position.y };
    }
    onChange({ ...record, layout });
  };

  return (
    <div className="relative flex h-full w-full flex-col" data-testid="schema-designer-canvas">
      <SchemaDesignerNodeContext.Provider value={onNodeUpdate}>
        <ReactFlow
          nodes={initialNodes}
          edges={initialEdges}
          nodeTypes={nodeTypes}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          defaultEdgeOptions={FK_EDGE_DEFAULTS}
          fitView
          proOptions={{ hideAttribution: true }}
        >
          <Background color="hsl(var(--border))" gap={40} size={1} />
          <Controls
            showInteractive={false}
            className="overflow-hidden rounded-md border border-border bg-panel text-fg"
          />
          <Panel position="top-left" className="!m-2 flex gap-1.5">
            <Button size="sm" variant="default" className="gap-1.5" onClick={addTable} aria-label="add table">
              <Plus size={13} /> Add table
            </Button>
            <Button size="sm" variant="outline" className="gap-1.5" onClick={autoLayout} aria-label="auto layout">
              <LayoutGrid size={13} /> Auto-layout
            </Button>
            {importing && (
              <span className="inline-flex items-center gap-1.5 rounded-md border border-border bg-panel px-2 py-1 text-xs text-muted">
                <Loader2 size={12} className="animate-spin" /> importing…
              </span>
            )}
          </Panel>
          <Panel position="top-right" className="!m-2">
            <div className="rounded-md border border-border bg-panel/95 px-2 py-1.5 text-[11px] text-muted shadow-sm">
              Drag a column handle → another column handle to create a relationship.
            </div>
          </Panel>
        </ReactFlow>
      </SchemaDesignerNodeContext.Provider>
    </div>
  );
}

/** Rename a key in the layout map (when a table is renamed inline, its geometry follows). */
function renameLayoutKey(
  layout: Record<string, { x: number; y: number }>,
  from: string,
  to: string,
): Record<string, { x: number; y: number }> {
  if (from === to) return layout;
  const next = { ...layout };
  if (next[from] !== undefined) {
    next[to] = next[from];
    delete next[from];
  }
  return next;
}

/** Merge the live node mutations (drag positions) back into the record. Column edits go through the
 *  Context callback (`onNodeUpdate`); this handles only the position changes xyflow reports. */
function mergeNodes(rec: DbSchemaRecord, nodes: Node<EditableTableNodeData>[]): DbSchemaRecord {
  const layout: Record<string, { x: number; y: number }> = { ...rec.layout };
  for (const n of nodes) layout[n.data.name] = { x: n.position.x, y: n.position.y };
  return { ...rec, layout };
}

/** Merge the live edge mutations back into the record's `fks` (declared edges only). */
function mergeEdges(rec: DbSchemaRecord, edges: Edge[]): DbSchemaRecord {
  const fks = edges
    .filter((e) => (e.data as { declared?: boolean })?.declared && e.sourceHandle && e.targetHandle)
    .map((e) => ({
      name: (e.data as { name?: string })?.name ?? "",
      fromTable: e.source,
      fromColumns: [e.sourceHandle!],
      toTable: e.target,
      toColumns: [e.targetHandle!],
    }));
  return { ...rec, fks };
}

/** A loose guess at the neutral type from a live catalog type string (import-from-source). Maps
 *  common SQL type names back to the canonical vocabulary; unknown types default to `text`. */
function guessNeutralType(liveType: string): string {
  const lc = liveType.toLowerCase();
  if (lc.includes("int")) return "integer";
  if (lc.includes("char") || lc.includes("text") || lc.includes("string")) return "text";
  if (lc.includes("float") || lc.includes("double") || lc.includes("real")) return "real";
  if (lc.includes("bool")) return "boolean";
  if (lc.includes("blob") || lc.includes("binary") || lc.includes("bytea")) return "blob";
  if (lc.includes("timestamp") || lc.includes("datetime")) return "timestamp";
  if (lc.includes("date")) return "date";
  if (lc.includes("numeric") || lc.includes("decimal")) return "numeric";
  if (lc.includes("json")) return "json";
  return "text";
}

void Upload;
