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

import { useCallback, useEffect, useMemo, useState } from "react";
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
import { mergeImport } from "./mergeImport";
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
  /** Callback when an import fails (bad source, denied, unreachable catalog). The page surfaces the
   *  message; without it a failed read would reject unhandled and the user would see nothing. */
  onImportError?: (message: string) => void;
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
  onImportError,
}: Props) {
  // Re-project when the record NAME changes (a different schema loaded) OR when the SET of table
  // names changes (a table was added/removed/imported). Live column edits mutate the graph in place
  // via the Context callback, so keying on the name-set — not the full record — avoids fighting the
  // author's focus/cursor on every keystroke while still surfacing added/imported tables (which
  // otherwise never appear: the import merges new tables under the same schema name). Sorted+joined
  // so the key is order-stable.
  // The canvas holds its own React Flow node/edge state (the FlowCanvas pattern) so drags land
  // immediately and STICK. A fully-controlled nodes={memo(record)} graph snaps every drag back,
  // because a memo keyed on the table-set cannot observe a position-only change. Local state is the
  // live graph; every mutation is mirrored back into the record via onChange.
  const [nodes, setNodes] = useState<Node<EditableTableNodeData>[]>(
    () => recordToFlow(record).nodes,
  );
  const [edges, setEdges] = useState<Edge[]>(() => recordToFlow(record).edges);

  // Re-seed from the record when a different schema loads (NAME change) OR the SET of table names
  // changes (a table added/removed/imported) - the two cases the local graph cannot derive itself.
  // Column edits and drags stay local (keyed off the name-set) so a keystroke or drag never wipes
  // the graph. Sorted+joined for an order-stable key.
  const tableKey = record.tables.map((t) => t.name).sort().join(" ");
  useEffect(() => {
    const flow = recordToFlow(record);
    setNodes(flow.nodes);
    setEdges(flow.edges);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [record.name, tableKey]);

  const nodeTypes = useMemo(
    () => ({ [SCHEMA_TABLE_NODE_TYPE]: EditableTableNode as unknown as typeof EditableTableNode }),
    [],
  );

  /** The node-edit callback the canvas provides via Context. A node reports an inline edit (rename,
   *  type change, add/remove column, toggle PK/nullable) → the canvas merges it into the record. */
  const onNodeUpdate = useCallback(
    (id: string, next: EditableTableNodeData) => {
      // Update the live node in place (so the edited columns/name re-render immediately) AND mirror
      // the change into the record. A column edit does NOT change the table-set, so the re-seed
      // effect won't fire — the local update here is what keeps the node visually current.
      setNodes((ns) =>
        ns.map((n) => (n.id === id ? { ...n, data: next } : n)),
      );
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
      // Apply to LOCAL state first (drags land + stick), then mirror positions into the record.
      const next = applyNodeChanges(changes, nodes) as Node<EditableTableNodeData>[];
      setNodes(next);
      onChange(mergeNodes(record, next));
    },
    [nodes, record, onChange],
  );
  const onEdgesChange = useCallback(
    (changes: EdgeChange[]) => {
      const next = applyEdgeChanges(changes, edges);
      setEdges(next);
      onChange(mergeEdges(record, next));
    },
    [edges, record, onChange],
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
      const next = addEdge(edge, edges);
      setEdges(next);
      onChange(mergeEdges(record, next));
    },
    [edges, record, onChange],
  );

  // The import-from-source path: read the live catalog's tables + columns and merge as new nodes.
  // Gated on `importSource` only — the page sets `importSource` and `importing:true` together, so
  // gating on `importing` here would short-circuit the effect before it ever ran (the spinner would
  // then hang forever, since nothing calls `onImportDone`). `importing` is the UI-only busy flag.
  useEffect(() => {
    if (!importSource) return;
    let alive = true;
    (async () => {
      try {
        const tables = await import("../../../lib/datasources").then((m) => m.discoverTables(importSource));
        if (!alive) return;
        const discovered = await Promise.all(
          tables.map(async (t) => ({
            name: t.name,
            columns: await describeTable(importSource, t.name),
          })),
        );
        if (!alive) return;
        // The whole merge — dedup, neutral-type mapping, layout seeding, and FK inference (the same
        // naming-convention inference the read-only ERD Diagram uses) — is the pure `mergeImport`.
        onChange(mergeImport(record, discovered));
      } catch (e) {
        // A failed catalog read (denied / bad source / no sidecar) must be reported, not left to
        // reject unhandled. Report only if still mounted for this source.
        if (alive) onImportError?.(e instanceof Error ? e.message : String(e));
      } finally {
        if (alive) onImportDone();
      }
    })();
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [importSource]);

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
    const flow = recordToFlow(record);
    const laid = layoutWithDagre(flow.nodes, flow.edges);
    const layout: Record<string, { x: number; y: number }> = { ...record.layout };
    for (const n of laid) {
      layout[n.data.name] = { x: n.position.x, y: n.position.y };
    }
    // Auto-layout only moves nodes (no table-set change) so the re-seed effect won't fire — push the
    // new positions into local state directly, and mirror the layout into the record.
    setNodes((ns) =>
      ns.map((n) => {
        const pos = layout[n.data.name];
        return pos ? { ...n, position: pos } : n;
      }),
    );
    onChange({ ...record, layout });
  };

  return (
    <div className="relative flex h-full w-full flex-col" data-testid="schema-designer-canvas">
      <SchemaDesignerNodeContext.Provider value={onNodeUpdate}>
        <ReactFlow
          nodes={nodes}
          edges={edges}
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

void Upload;
