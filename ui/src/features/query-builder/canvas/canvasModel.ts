// Adapted from Tabularis (github.com/TabularisDB/tabularis), Apache-2.0. Interaction design preserved;
// the data layer is rewired onto our typed SqlBuilderQuery (model-as-truth, not nodes-as-truth).
//
// The pure projection between the typed `SqlBuilderQuery` and React-Flow's `{nodes, edges}` (the
// visual-canvas-builder slice). The model is the single source of truth; the canvas is a projection
// re-derived on every edit. Node positions are view state (carried in the opaque `builderLayout`
// blob), never query semantics. One responsibility per file (FILE-LAYOUT): this is the only seam
// between the typed query and React-Flow's node/edge shape — no React here.

import type { Schema } from "@/lib/schema";
import type { SqlBuilderQuery, SqlJoin, SqlJoinType } from "@/lib/panel-kit/sql/query";

/** One column shown on a table node. */
export interface CanvasColumn {
  name: string;
  type: string;
}

/** React-Flow node `data` payload for a table node (consumed by `TableNode.tsx`). Carries an index
 *  signature so it satisfies React-Flow's `Record<string, unknown>` requirement (the host spreads
 *  additional runtime fields — callbacks, selection state — onto `data` before passing to ReactFlow). */
export interface CanvasNodeData extends Record<string, unknown> {
  table: string;
  columns: CanvasColumn[];
}

/** A React-Flow table node. `id` is the table name (so a connect event can resolve back to a join). */
export interface CanvasNode {
  id: string;
  type: "table";
  position: { x: number; y: number };
  data: CanvasNodeData;
}

/** A React-Flow join edge. `sourceHandle`/`targetHandle` carry the connected column names (the join's
 *  ON keys); for a CROSS join (no `on`) the handles are absent. */
export interface CanvasEdge {
  id: string;
  source: string;
  target: string;
  sourceHandle: string;
  targetHandle: string;
  data: { joinType: SqlJoinType };
}

/** The React-Flow view of a `SqlBuilderQuery`. */
export interface CanvasModel {
  nodes: CanvasNode[];
  edges: CanvasEdge[];
}

/** The shape of the opaque `SqlSourceState.builderLayout` blob — `{ [table]: {x,y} }`. Treated as
 *  `unknown` everywhere outside this file (the model is the only place that interprets it). */
export type LayoutBlob = Record<string, { x: number; y: number }>;

const GRID_STEP_X = 280;
const GRID_Y = 40;

/** Coerce the opaque persisted layout blob into the typed shape (or `null` if absent/malformed). */
function readLayout(layout: unknown): LayoutBlob | null {
  if (!layout || typeof layout !== "object") return null;
  const obj = layout as Record<string, unknown>;
  const out: LayoutBlob = {};
  for (const [k, v] of Object.entries(obj)) {
    if (v && typeof v === "object" && typeof (v as { x?: unknown }).x === "number" && typeof (v as { y?: unknown }).y === "number") {
      out[k] = { x: (v as { x: number }).x, y: (v as { y: number }).y };
    }
  }
  return Object.keys(out).length > 0 ? out : null;
}

/** Look up a table's columns in the schema; `[]` if the table isn't in the schema (the host may not
 *  have described it yet — the node still renders with just a name). */
function columnsOf(schema: Schema, table: string): CanvasColumn[] {
  const t = schema.tables.find((x) => x.name === table);
  return t ? t.columns.map((c) => ({ name: c.name, type: c.type })) : [];
}

/** The position of `table` from the layout blob, or the auto-grid fallback (x = index*step, y = 40). */
function positionFor(layout: LayoutBlob | null, table: string, index: number): { x: number; y: number } {
  if (layout && layout[table]) return layout[table];
  return { x: index * GRID_STEP_X, y: GRID_Y };
}

/** Derive the canvas view from the typed query + schema + persisted layout blob. One node per table
 *  (FROM + each join table), one edge per join (CROSS joins carry no handles). */
export function toFlow(query: SqlBuilderQuery, schema: Schema, layout?: unknown): CanvasModel {
  if (!query.table.trim()) return { nodes: [], edges: [] };

  const parsed = readLayout(layout);
  // FROM table first, then each joined table — index drives the auto-grid fallback.
  const tables = [query.table, ...(query.joins ?? []).map((j) => j.table)];
  const nodes: CanvasNode[] = tables.map((table, index) => ({
    id: table,
    type: "table",
    position: positionFor(parsed, table, index),
    data: { table, columns: columnsOf(schema, table) },
  }));

  const edges: CanvasEdge[] = [];
  for (const j of query.joins ?? []) {
    const firstOn = j.on?.[0];
    const edge: CanvasEdge = {
      id: edgeId(j),
      source: firstOn?.leftTable ?? query.table,
      target: j.table,
      sourceHandle: firstOn?.leftColumn ?? "",
      targetHandle: firstOn?.rightColumn ?? "",
      data: { joinType: j.type },
    };
    edges.push(edge);
  }
  return { nodes, edges };
}

/** The stable edge id for a join (keyed by the joined table — a table joins at most once in v1). */
export function edgeId(join: SqlJoin): string {
  return `e:${join.table}`;
}

/** Map a connect event (two column handles) to a `SqlJoin` edit. The source is whichever handle the
 *  drag started from; the target is what it landed on. `leftTable` is set only when the source table
 *  isn't the FROM table (so ≥2 joins resolve their left side correctly). Type defaults to `inner`. */
export function joinFromConnect(
  source: { table: string; column: string },
  target: { table: string; column: string },
  type: SqlJoinType = "inner",
  fromTable?: string,
): SqlJoin {
  const leftTable = fromTable && source.table !== fromTable ? source.table : undefined;
  return {
    table: target.table,
    type,
    on: [{ leftTable, leftColumn: source.column, rightColumn: target.column }],
  };
}

/** Map the current node positions back to a layout blob (`{ [table]: {x,y} }`). The opaque blob to
 *  persist on `SqlSourceState.builderLayout` — never touches the semantic query. */
export function layoutFromNodes(nodes: CanvasNode[]): unknown {
  const out: LayoutBlob = {};
  for (const n of nodes) out[n.data.table] = n.position;
  return out;
}
