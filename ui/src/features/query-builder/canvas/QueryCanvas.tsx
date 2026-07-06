// Adapted from Tabularis (github.com/TabularisDB/tabularis), Apache-2.0. Interaction design preserved;
// the data layer is rewired onto our typed SqlBuilderQuery (model-as-truth, not nodes-as-truth).
//
// The React-Flow canvas host (visual-canvas-builder slice). Renders one node per table (FROM + each
// join), one edge per join. Drag column→column to append a `SqlJoin`; click an edge label to cycle its
// type; drag a node to reposition (writes the opaque `builderLayout` blob on drag stop). An "Add
// table" dropdown above the canvas appends a (default INNER, empty-`on`) join the user then wires by
// dragging column-to-column. Reads/writes `SqlBuilderQuery` only — the model is the source of truth.

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  applyEdgeChanges,
  applyNodeChanges,
  Background,
  Controls,
  MiniMap,
  ReactFlow,
  ReactFlowProvider,
  type Connection,
  type Edge,
  type EdgeChange,
  type Node,
  type NodeChange,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import { Button } from "@/components/ui/button";
import type { Schema } from "@/lib/schema";
import type {
  SqlAggregation,
  SqlBuilderQuery,
  SqlJoin,
} from "@/lib/panel-kit/sql/query";
import {
  disconnectJoin,
  joinFromConnect,
  layoutFromNodes,
  removeTable,
  toFlow,
  type CanvasNode,
} from "./canvasModel";
import { JoinEdge, nextJoinType, type JoinEdgeData } from "./JoinEdge";
import { TableNode, type TableNodeData } from "./TableNode";

const nodeTypes = { table: TableNode };
const edgeTypes = { join: JoinEdge };

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2 text-[11px] text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

interface Props {
  schema: Schema;
  query: SqlBuilderQuery;
  onChange: (query: SqlBuilderQuery) => void;
  /** The opaque layout blob persisted on `SqlSourceState.builderLayout` (node positions). */
  layout?: unknown;
  /** Persist a new layout blob (on node drag-stop). The host stores it on `SqlSourceState.builderLayout`. */
  onLayoutChange?: (layout: unknown) => void;
}

/** The canvas host. Wrap in `<ReactFlowProvider>` (exported below) so `JoinEdge` can useReactFlow. */
function QueryCanvasInner({ schema, query, onChange, layout, onLayoutChange }: Props) {
  const view = useMemo(() => toFlow(query, schema, layout), [query, schema, layout]);

  const [nodes, setNodes] = useState<Node[]>(() => view.nodes as Node[]);
  const [edges, setEdges] = useState<Edge[]>(() => view.edges as Edge[]);

  // Re-seed NODES when the table set (or schema) changes — positions are restored from the layout
  // blob. Re-seeding on every query change would fight an in-flight drag; the structure key
  // isolates that. (Column arrival and pending-state changes flow through `paintedNodes` fresh
  // every render — no reseed needed.)
  const structureKey = useMemo(
    () =>
      JSON.stringify({
        table: query.table,
        joins: (query.joins ?? []).map((j) => j.table),
        schemaTables: schema.tables.map((t) => t.name),
      }),
    [query.table, query.joins, schema],
  );
  useEffect(() => {
    setNodes(view.nodes as Node[]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [structureKey]);

  // Re-seed EDGES whenever the model's join wiring changes (connect / disconnect / type cycle) —
  // edges carry no drag state, so the model-derived set (with the canonical `e:<table>` ids the
  // cycle handler parses) can always win.
  const edgeKey = useMemo(() => JSON.stringify(view.edges), [view.edges]);
  useEffect(() => {
    setEdges(view.edges as Edge[]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [edgeKey]);

  /** Node changes: a `remove` (keyboard delete) maps to the typed remove-table edit; everything
   *  else (drag, select, dimensions) applies to local view state only. */
  const onNodesChange = useCallback(
    (changes: NodeChange[]) => {
      let next = query;
      for (const c of changes) {
        if (c.type === "remove" && c.id !== query.table) next = removeTable(next, c.id);
      }
      if (next !== query) onChange(next);
      const rest = changes.filter((c) => c.type !== "remove" || c.id !== query.table);
      setNodes((ns) => applyNodeChanges(rest, ns));
    },
    [query, onChange],
  );
  /** Edge changes: a `remove` (keyboard delete on a selected edge) clears the join's ON keys —
   *  the table stays on the canvas as a pending ("not joined") node, out of the emitted SQL. */
  const onEdgesChange = useCallback(
    (changes: EdgeChange[]) => {
      let next = query;
      for (const c of changes) {
        if (c.type === "remove" && c.id.startsWith("e:")) next = disconnectJoin(next, c.id.slice(2));
      }
      if (next !== query) onChange(next);
      setEdges((es) => applyEdgeChanges(changes, es));
    },
    [query, onChange],
  );

  /** Drop a new join onto the query — the dragged connection maps to a typed `SqlJoin`. */
  const onConnect = useCallback(
    (c: Connection) => {
      if (!c.source || !c.target || !c.sourceHandle || !c.targetHandle) return;
      const sourceTable = c.source;
      const targetTable = c.target;
      // If the target table is already joined, replace its `on`; else append a new join.
      const existing = (query.joins ?? []).find((j) => j.table === targetTable);
      const newJoin = joinFromConnect(
        { table: sourceTable, column: c.sourceHandle },
        { table: targetTable, column: c.targetHandle },
        existing?.type ?? "inner",
        query.table,
      );
      let nextJoins: SqlJoin[];
      if (existing) {
        nextJoins = (query.joins ?? []).map((j) => (j.table === targetTable ? { ...newJoin, type: existing.type } : j));
      } else {
        nextJoins = [...(query.joins ?? []), newJoin];
      }
      // The edge itself re-derives from the model (the edgeKey reseed) with its canonical
      // `e:<table>` id — no local addEdge, or the cycle handler couldn't resolve the join.
      onChange({ ...query, joins: nextJoins });
    },
    [query, onChange],
  );

  /** Persist node positions on drag-stop (the opaque layout blob — never touches the semantic query). */
  const onNodeDragStop = useCallback(
    () => onLayoutChange?.(layoutFromNodes(nodes as unknown as CanvasNode[])),
    [nodes, onLayoutChange],
  );

  /** Click-to-cycle a join's type — the edge label calls back into this. */
  const onCycleEdge = useCallback(
    (edgeId: string) => {
      // The edge id is `e:<table>` — find the join by table.
      const table = edgeId.startsWith("e:") ? edgeId.slice(2) : edgeId;
      const joins = query.joins ?? [];
      const next = joins.map((j) =>
        j.table === table ? { ...j, type: nextJoinType(j.type) } : j,
      );
      onChange({ ...query, joins: next });
    },
    [query, onChange],
  );

  /** Remove a table node — drops its join and every column/filter/sort/group-by referencing it. */
  const onDeleteNode = useCallback(
    (table: string) => {
      if (table === query.table) return; // the FROM table can't be removed from the canvas
      onChange(removeTable(query, table));
    },
    [query, onChange],
  );

  // The runtime overlay — column selection state (from query.columns) + callbacks (React fns).
  // The model is the structural truth; this enriches it for the node component.
  const paintedNodes = useMemo<Node[]>(() => {
    return nodes.map((n) => {
      const structural = view.nodes.find((x) => x.id === n.id);
      if (!structural) return n;
      const table = structural.data.table;
      const columnStates = buildColumnStates(structural.data.columns.map((c) => c.name), query.columns, table, query.table);
      const data: TableNodeData = {
        ...structural.data,
        columnStates,
        isJoinable: true,
        onColumnCheck: (column, checked) => onColumnCheck(query, table, column, checked, onChange),
        onColumnAggregation: (column, aggregation) =>
          onColumnAggregation(query, table, column, aggregation, onChange),
        onColumnAlias: (column, alias) => onColumnAlias(query, table, column, alias, onChange),
        onColumnOrder: (column, order) => onColumnOrder(query, table, column, order, onChange),
        onDelete: table === query.table ? undefined : () => onDeleteNode(table),
      };
      // Preserve the live drag position from `n.position` (the model-derived position only seeds).
      return { ...n, data: data as unknown as Node["data"] };
    });
  }, [nodes, view.nodes, query, onChange, onDeleteNode]);

  const paintedEdges = useMemo<Edge[]>(
    () =>
      edges.map((e) => ({
        ...e,
        type: "join",
        data: { joinType: (e.data as JoinEdgeData | undefined)?.joinType ?? "inner", onCycle: onCycleEdge } as JoinEdgeData,
      })),
    [edges, onCycleEdge],
  );

  /** Add-table dropdown state — the user picks a schema table to drop a new (empty-`on`) join node. */
  const [pickedTable, setPickedTable] = useState("");
  const addableTables = schema.tables
    .map((t) => t.name)
    .filter((name) => name !== query.table && !(query.joins ?? []).some((j) => j.table === name));

  const addTable = () => {
    if (!pickedTable) return;
    if (!query.table) {
      onChange({ ...query, table: pickedTable });
    } else {
      // Append a join with an empty `on` — the user then drags column→column to set the keys.
      const join: SqlJoin = { table: pickedTable, type: "inner", on: [] };
      onChange({ ...query, joins: [...(query.joins ?? []), join] });
    }
    setPickedTable("");
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-1.5 border-b border-border bg-panel px-2 py-1.5">
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
        <select
          aria-label="add table"
          className={FIELD}
          value={pickedTable}
          onChange={(e) => setPickedTable(e.target.value)}
        >
          <option value="">+ add table…</option>
          {addableTables.map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </select>
        <Button type="button" variant="ghost" size="sm" onClick={addTable} disabled={!pickedTable} className="h-7 text-[11px] text-muted">
          Add
        </Button>
      </div>
      <div className="relative min-h-0 flex-1">
        {/* Discoverability hints — the join gesture is invisible until you know it exists. */}
        {nodes.length === 0 && (
          <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center text-xs text-muted">
            Pick a table above to start the query.
          </div>
        )}
        {nodes.length >= 2 && edges.length === 0 && (
          <div className="pointer-events-none absolute inset-x-0 top-2 z-10 flex justify-center">
            <span className="rounded-md border border-border bg-panel/90 px-2.5 py-1 text-[11px] text-muted shadow">
              Drag from a column dot on one table to a column on another to create a join.
            </span>
          </div>
        )}
        <ReactFlow
          nodes={paintedNodes}
          edges={paintedEdges}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          onNodeDragStop={onNodeDragStop}
          fitView
          defaultEdgeOptions={{ animated: true, style: { stroke: "hsl(var(--accent))", strokeWidth: 2 } }}
        >
          <Background gap={16} size={1} color="hsl(var(--border))" />
          <Controls showInteractive={false} />
          <MiniMap
            pannable
            zoomable
            nodeColor={() => "hsl(var(--accent))"}
            maskColor="hsl(var(--bg)/0.6)"
            style={{ backgroundColor: "hsl(var(--panel))" }}
          />
        </ReactFlow>
      </div>
    </div>
  );
}

/** The canvas host wrapped in `<ReactFlowProvider>` (so JoinEdge can `useReactFlow`). */
export function QueryCanvas(props: Props) {
  return (
    <ReactFlowProvider>
      <QueryCanvasInner {...props} />
    </ReactFlowProvider>
  );
}

/** Build the per-column state lookup for a table node (selected/aggregation/alias/order). An
 *  unqualified column belongs to the FROM table (`SqlColumn.table` "Defaults to the FROM table") —
 *  e.g. a Code→Builder-parsed `SELECT "name" FROM "site" …` must tick `name` on the `site` node. */
function buildColumnStates(
  catalog: string[],
  queryColumns: SqlBuilderQuery["columns"],
  table: string,
  fromTable: string,
): TableNodeData["columnStates"] {
  const out: TableNodeData["columnStates"] = {};
  for (const name of catalog) out[name] = { selected: false };
  for (const c of queryColumns) {
    if ((c.table ?? fromTable) !== table) continue;
    if (c.name === "*") continue;
    if (!(c.name in out)) out[c.name] = { selected: false };
    out[c.name] = {
      selected: true,
      aggregation: c.aggregation,
      alias: c.alias,
      order: c.order,
    };
  }
  return out;
}

/** Toggle a column's selection — adds/removes the matching `SqlColumn` (qualified by `table`). */
function onColumnCheck(
  query: SqlBuilderQuery,
  table: string,
  column: string,
  checked: boolean,
  onChange: (q: SqlBuilderQuery) => void,
) {
  const others = query.columns.filter((c) => !(c.name === column && (c.table ?? query.table) === table));
  if (!checked) {
    onChange({ ...query, columns: others });
    return;
  }
  onChange({ ...query, columns: [...others, { name: column, table }] });
}

/** Set/unset a column's aggregation. */
function onColumnAggregation(
  query: SqlBuilderQuery,
  table: string,
  column: string,
  aggregation: SqlAggregation | undefined,
  onChange: (q: SqlBuilderQuery) => void,
) {
  const next = query.columns.map((c) =>
    c.name === column && (c.table ?? query.table) === table ? { ...c, aggregation } : c,
  );
  onChange({ ...query, columns: next });
}

/** Set/clear a column's alias. */
function onColumnAlias(
  query: SqlBuilderQuery,
  table: string,
  column: string,
  alias: string | undefined,
  onChange: (q: SqlBuilderQuery) => void,
) {
  const next = query.columns.map((c) =>
    c.name === column && (c.table ?? query.table) === table ? { ...c, alias } : c,
  );
  onChange({ ...query, columns: next });
}

/** Set/clear a column's SELECT position (stable ordering). */
function onColumnOrder(
  query: SqlBuilderQuery,
  table: string,
  column: string,
  order: number | undefined,
  onChange: (q: SqlBuilderQuery) => void,
) {
  const next = query.columns.map((c) =>
    c.name === column && (c.table ?? query.table) === table ? { ...c, order } : c,
  );
  onChange({ ...query, columns: next });
}
