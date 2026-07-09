// Pure projection of a `DbSchemaRecord` ⇄ React Flow graph (schema-designer scope). Separated from
// the canvas so the projection is unit-testable (no React, no IO) — same convention as the ERD's
// `schemaToFlow.ts`. One responsibility, one file (FILE-LAYOUT).
//
// **Provenance:** the editable-table-node shape is adapted from tabularis's read-only
// `SchemaTableNode` (Apache-2.0) — the handles + column-row layout are lifted from its skeleton;
// editing (inline inputs, add/remove, PK toggle) is original. ChartDB is AGPL-3.0 — referenced for
// UX only, no code copied (scope Non-goals, license hygiene).

import type { Edge, Node } from "@xyflow/react";

import type { DbSchemaRecord, DesignFk } from "@/lib/datasources";

/** The per-node payload React Flow hands to the editable table node. */
export interface EditableTableNodeData extends Record<string, unknown> {
  name: string;
  columns: { name: string; type: string; nullable: boolean; pk: boolean }[];
}

/** Build the React Flow nodes + edges from a record. Positions come from the record's layout
 *  (tables not in the layout land at {0,0} — the dagre pass moves them). Edges are the declared
 *  FKs (solid, labeled with the from-column), drawn source-handle → target-handle per column. */
export function recordToFlow(rec: DbSchemaRecord): {
  nodes: Node<EditableTableNodeData>[];
  edges: Edge[];
} {
  const pkByTable = new Map<string, Set<string>>();
  for (const t of rec.tables) {
    pkByTable.set(t.name, new Set(t.pk));
  }
  const nodes: Node<EditableTableNodeData>[] = rec.tables.map((t) => {
    const pk = pkByTable.get(t.name) ?? new Set<string>();
    const pos = rec.layout[t.name] ?? { x: 0, y: 0 };
    return {
      id: t.name,
      type: "editableTable",
      position: pos,
      data: {
        name: t.name,
        columns: t.columns.map((c) => ({
          name: c.name,
          type: c.type,
          nullable: c.nullable,
          pk: pk.has(c.name),
        })),
      },
    };
  });

  const edges: Edge[] = rec.fks.map((fk, i) => fkEdge(fk, i));
  return { nodes, edges };
}

/** One declared FK → one React Flow edge (source = from-table's column handle, target = to-table's
 *  column handle). Solid (a DECLARED FK — not the dashed naming-convention guess the read-only ERD
 *  uses). */
function fkEdge(fk: DesignFk, index: number): Edge {
  const fromCol = fk.fromColumns[0] ?? "_";
  const toCol = fk.toColumns[0] ?? "_";
  return {
    id: `fk:${fk.fromTable}:${fromCol}->${fk.toTable}:${toCol}#${index}`,
    source: fk.fromTable,
    sourceHandle: fromCol,
    target: fk.toTable,
    targetHandle: toCol,
    type: "smoothstep",
    animated: false,
    style: { stroke: "hsl(var(--accent))", strokeWidth: 1.5 },
    label: fromCol,
    labelBgPadding: [4, 2],
    labelBgBorderRadius: 3,
    labelBgStyle: { fill: "hsl(var(--bg))", fillOpacity: 0.9 },
    labelStyle: {
      fill: "hsl(var(--muted))",
      fontSize: 10,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
    },
    data: { declared: true, name: fk.name, onDelete: fk.onDelete },
  };
}

/** Project the current nodes + edges back into a `DbSchemaRecord` (the save shape). The node's
 *  data columns carry `pk` (a UI toggle) — projected into the table's `pk` array. Node positions
 *  become the record's `layout`. FK edges become the record's `fks`. */
export function flowToRecord(
  name: string,
  nodes: Node<EditableTableNodeData>[],
  edges: Edge[],
): DbSchemaRecord {
  const tables = nodes.map((n) => {
    const pk = n.data.columns.filter((c) => c.pk).map((c) => c.name);
    return {
      name: n.data.name,
      pk,
      columns: n.data.columns.map((c) => ({
        name: c.name,
        type: c.type,
        nullable: c.nullable,
      })),
    };
  });
  const layout: Record<string, { x: number; y: number }> = {};
  for (const n of nodes) {
    layout[n.data.name] = { x: n.position.x, y: n.position.y };
  }
  const fks = edges
    .filter((e) => e.sourceHandle && e.targetHandle && (e.data as { declared?: boolean })?.declared)
    .map((e) => ({
      name: ((e.data as { name?: string })?.name) ?? "",
      fromTable: e.source,
      fromColumns: [e.sourceHandle!],
      toTable: e.target,
      toColumns: [e.targetHandle!],
      onDelete: (e.data as { onDelete?: string })?.onDelete,
    }));
  return { name, version: 1, tables, fks, layout };
}
