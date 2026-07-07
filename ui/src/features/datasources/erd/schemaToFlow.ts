// Pure projection of a datasource's discovered schema (the `federation.schema` tables + columns) into a
// React Flow graph: one node per table, one edge per INFERRED relationship. `federation.schema` returns
// NO foreign-key metadata (just column names + types), so relationships are guessed by the two naming
// conventions callers actually use here — `<table>_id` or `<table>Ref` → that table — and every such
// edge is marked `inferred` so the canvas never fabricates certainty (datasources-ux ERD scope). Pure
// (no React, no IO) → unit-testable. One responsibility, one file (FILE-LAYOUT).

import type { Edge, Node } from "@xyflow/react";

import type { DbColumn } from "@/lib/datasources";

/** One table's full schema — what the ERD is projected from. */
export interface ErdTable {
  name: string;
  columns: DbColumn[];
}

/** The per-node payload React Flow hands to {@link SchemaTableNode}. */
export interface SchemaTableNodeData extends Record<string, unknown> {
  name: string;
  columns: DbColumn[];
}

/** Why an edge exists. Always a naming-convention guess here — never a declared FK. */
export type ErdReason = "suffix _id" | "suffix Ref";

/** An inferred relationship: the child table owns a ref column pointing at the parent table. */
export interface ErdRelation {
  /** Child table (the one that holds the ref column). React Flow edge `source`. */
  source: string;
  /** The ref column name — the edge leaves this handle on the child. */
  sourceHandle: string;
  /** Referenced (parent) table. React Flow edge `target`. */
  target: string;
  /** The parent's `id` column handle when it has one, else undefined (node-level attach). */
  targetHandle: string | undefined;
  reason: ErdReason;
}

export interface ErdFlow {
  nodes: Node<SchemaTableNodeData>[];
  edges: Edge[];
  /** The relations the edges were inferred from (drives the legend + tests). */
  relations: ErdRelation[];
}

/** The column a parent is referenced by when it owns the identity column. */
const ID_HANDLE = "id";

/** Strip a trailing `_id` or `Ref` suffix and return the candidate target table + reason, else null.
 *  `id` / `_id` / `Ref` themselves are too short to be a real ref (need ≥1 char before the suffix) and
 *  are skipped, so an identity `id` column never produces a self-edge. */
function refTarget(col: string): { candidate: string; reason: ErdReason } | null {
  if (col.length > 3 && col.endsWith("_id")) {
    return { candidate: col.slice(0, -3), reason: "suffix _id" };
  }
  if (col.length > 3 && col.endsWith("Ref")) {
    return { candidate: col.slice(0, -3), reason: "suffix Ref" };
  }
  return null;
}

/** Infer child → parent relations by naming convention. Pure + deterministic. A case-insensitive name
 *  → table index lets `siteRef` resolve to table `site`; a candidate with NO matching table yields
 *  nothing (we never invent a node or fabricate certainty). */
export function inferRelations(tables: ErdTable[]): ErdRelation[] {
  const byName = new Map<string, string>(); // lowercased name → real name
  for (const t of tables) byName.set(t.name.toLowerCase(), t.name);
  const columnsById = new Map<string, Set<string>>();
  for (const t of tables) columnsById.set(t.name, new Set(t.columns.map((c) => c.name)));

  const rels: ErdRelation[] = [];
  for (const child of tables) {
    for (const col of child.columns) {
      const m = refTarget(col.name);
      if (!m) continue;
      const parent = byName.get(m.candidate.toLowerCase());
      if (!parent) continue; // no such table → no edge
      rels.push({
        source: child.name,
        sourceHandle: col.name,
        target: parent,
        targetHandle: columnsById.get(parent)?.has(ID_HANDLE) ? ID_HANDLE : undefined,
        reason: m.reason,
      });
    }
  }
  return rels;
}

/** Project the discovered schema → React Flow nodes + edges. Each node carries its columns so
 *  {@link SchemaTableNode} renders them; edges are the inferred relations, drawn dashed so the canvas
 *  reads them as guesses, not declared FKs. Positions are left at {0,0} — {@link layoutErd} places them. */
export function schemaToFlow(tables: ErdTable[]): ErdFlow {
  const nodes: Node<SchemaTableNodeData>[] = tables.map((t) => ({
    id: t.name,
    type: "schemaTable",
    position: { x: 0, y: 0 },
    data: { name: t.name, columns: t.columns },
  }));

  const relations = inferRelations(tables);
  const edges: Edge[] = relations.map((r, i) => ({
    id: `erd:${r.source}:${r.sourceHandle}->${r.target}:${r.targetHandle ?? "_"}#${i}`,
    source: r.source,
    sourceHandle: r.sourceHandle,
    target: r.target,
    targetHandle: r.targetHandle,
    type: "smoothstep",
    animated: false,
    // Dashed = inferred (the legend says so). Never a solid declared-FK look.
    style: { stroke: "hsl(var(--muted))", strokeWidth: 1.3, strokeDasharray: "5 3" },
    label: r.sourceHandle,
    labelBgPadding: [4, 2],
    labelBgBorderRadius: 3,
    labelBgStyle: { fill: "hsl(var(--bg))", fillOpacity: 0.9 },
    labelStyle: {
      fill: "hsl(var(--muted))",
      fontSize: 10,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
    },
    data: { inferred: true, reason: r.reason },
  }));

  return { nodes, edges, relations };
}
