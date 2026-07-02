// Flow import/export (flows-canvas scope) — the JSON round-trip, extracted from FlowCanvas so the
// canvas file owns rendering, not file plumbing (FILE-LAYOUT). Export downloads the flow as JSON with
// a derived top-level `edges: [{from,to}]` for legibility (the "I can't see the connections" report);
// import re-derives the graph from each node's canonical `needs` and ignores that informational field.

import type { Flow } from "@/lib/flows";

/** Trigger a browser download of `flow` as pretty JSON, augmented with a derived `edges` list. The
 *  canonical connection data is each node's `needs`; `edges` is informational (import ignores it). */
export function downloadFlow(flow: Flow): void {
  const edges = flow.nodes.flatMap((n) => (n.needs ?? []).map((from) => ({ from, to: n.id })));
  const blob = new Blob([JSON.stringify({ ...flow, edges }, null, 2)], {
    type: "application/json",
  });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `${flow.id}.json`;
  a.click();
  URL.revokeObjectURL(url);
}

/** Parse an uploaded file into a Flow, pinning it to the OPEN flow's id + workspace (an import
 *  replaces the open flow's graph, it does not fork a new record). Throws on invalid JSON — the caller
 *  surfaces the message inline. The result is then re-validated through the real `flows.save` path. */
export async function parseImportedFlow(file: File, into: Flow): Promise<Flow> {
  const text = await file.text();
  const imported = JSON.parse(text) as Flow;
  return { ...imported, id: into.id, workspace: into.workspace };
}
