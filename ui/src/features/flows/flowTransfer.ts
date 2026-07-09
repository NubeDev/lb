// Flow import/export (flows-canvas scope; dialog-ified per flow-ui-polish) — the JSON round-trip,
// extracted from FlowCanvas so the canvas file owns rendering, not file plumbing (FILE-LAYOUT).
// Export serialises the flow (optionally just a node selection) with a derived top-level
// `edges: [{from,to}]` for legibility (the "I can't see the connections" report); import re-derives
// the graph from each node's canonical `needs` and ignores that informational field.

import type { Flow } from "@/lib/flows";

export interface FlowJsonOptions {
  /** Pretty-print (2-space) vs compact. */
  pretty: boolean;
  /** Export only these node ids (Node-RED's "selected nodes"). `needs` edges pointing OUTSIDE the
   *  selection are stripped (the exported fragment must stand alone); undefined = the whole flow. */
  selection?: Set<string>;
}

/** Serialise `flow` to its export JSON. The canonical connection data is each node's `needs`;
 *  the derived `edges` list is informational (import ignores it). */
export function flowToJson(flow: Flow, opts: FlowJsonOptions): string {
  const nodes = opts.selection
    ? flow.nodes
        .filter((n) => opts.selection!.has(n.id))
        .map((n) => ({ ...n, needs: (n.needs ?? []).filter((d) => opts.selection!.has(d)) }))
    : flow.nodes;
  const edges = nodes.flatMap((n) => (n.needs ?? []).map((from) => ({ from, to: n.id })));
  const doc = { ...flow, nodes, edges };
  return opts.pretty ? JSON.stringify(doc, null, 2) : JSON.stringify(doc);
}

/** Count the `needs` references a selection would strip (edges into the selection from outside) —
 *  surfaced as a warning in the export dialog so the strip is never silent. */
export function strippedNeedsCount(flow: Flow, selection: Set<string>): number {
  return flow.nodes
    .filter((n) => selection.has(n.id))
    .flatMap((n) => n.needs ?? [])
    .filter((d) => !selection.has(d)).length;
}

/** Trigger a browser download of `json` as `<id>.json`. */
export function downloadFlowJson(flowId: string, json: string): void {
  const blob = new Blob([json], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `${flowId}.json`;
  a.click();
  URL.revokeObjectURL(url);
}

/** Parse flow JSON text into a Flow, pinning it to the OPEN flow's id + workspace (an import
 *  replaces the open flow's graph, it does not fork a new record). Throws on invalid JSON or a
 *  node-less document — the caller surfaces the message inline. The result is then re-validated
 *  through the real `flows.save` path (schema + DAG). */
export function parseFlowJson(text: string, into: Flow): Flow {
  const imported = JSON.parse(text) as Flow;
  if (!Array.isArray(imported.nodes)) throw new Error("not a flow export (no `nodes` list)");
  return { ...imported, id: into.id, workspace: into.workspace };
}

/** Parse an uploaded file into a Flow (the file flavour of `parseFlowJson`). */
export async function parseImportedFlow(file: File, into: Flow): Promise<Flow> {
  return parseFlowJson(await file.text(), into);
}
