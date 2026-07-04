// Derive the cap-gated nav entries from `ext.list` — the app mirror of the web shell's
// `useExtensionPages` filter, plus an explicit cap gate: an entry whose declared bridge `scope`
// the session token does not hold is hidden. This is a CONVENIENCE only (no dead nav items) —
// the gateway re-checks every call server-side; the UI gate is never the security boundary.

import type { ExtRow, ExtUi } from "./ext.types";

/** One extension page available in the app's nav. */
export interface ExtNavEntry {
  ext: string;
  ui: ExtUi;
}

/** Keep enabled rows that declare a UI entry whose scope the session's caps satisfy. */
export function extNavEntries(rows: ExtRow[], caps: string[]): ExtNavEntry[] {
  return rows
    .filter((r) => r.enabled && r.ui && r.ui.entry)
    .filter((r) => (r.ui as ExtUi).scope.every((tool) => holdsToolCap(caps, tool)))
    .map((r) => ({ ext: r.ext, ui: r.ui as ExtUi }));
}

/** Does the cap set admit calling MCP `tool`? Mirrors the host grammar's wildcard segments
 *  (`mcp:proof-panel.*:call`) at display granularity — segment-wise `*` match on the tool part. */
export function holdsToolCap(caps: string[], tool: string): boolean {
  return caps.some((cap) => {
    const m = /^mcp:(.+):call$/.exec(cap);
    if (!m) return false;
    return segmentsMatch(m[1].split("."), tool.split("."));
  });
}

function segmentsMatch(pattern: string[], value: string[]): boolean {
  if (pattern.length === 1 && pattern[0] === "*") return true;
  if (pattern.length !== value.length) return false;
  return pattern.every((p, i) => p === "*" || p === value[i]);
}
