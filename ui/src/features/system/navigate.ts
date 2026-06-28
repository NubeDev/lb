// The one map from a subsystem `id` to the shell page that owns it (system-map scope) — so a status
// card can drill into the real view (the outbox grid, the extensions console, the DB browser) instead
// of being a dead end. One responsibility per file (FILE-LAYOUT): the id→surface lookup, nothing else.
//
// Only subsystems with a first-class page appear here. `gateway`/`bus` have no dedicated page (they
// are the transport itself), so they return null and open the in-page detail sheet instead. `store`/
// `ingest` → the data-console pages; `registry`/`extensions` → the Extensions console; `inbox`/
// `outbox` → their workflow pages; `mcp`/`acp` → their service pages (tool-catalog scope).

import type { CoreSurface } from "@/features/shell";

const SUBSYSTEM_SURFACE: Record<string, CoreSurface> = {
  store: "data",
  ingest: "ingest",
  inbox: "inbox",
  outbox: "outbox",
  extensions: "extensions",
  registry: "extensions",
  // The MCP + ACP runtime cards now own dedicated service pages (tool-catalog scope) — the catalog of
  // reachable tools, and the ACP adapter's static facts — so they drill there instead of opening the
  // generic detail sheet.
  mcp: "system-mcp",
  acp: "system-acp",
};

/** The shell surface that owns `subsystemId`, or `null` if it has no dedicated page. */
export function surfaceForSubsystem(subsystemId: string): CoreSurface | null {
  return SUBSYSTEM_SURFACE[subsystemId] ?? null;
}
