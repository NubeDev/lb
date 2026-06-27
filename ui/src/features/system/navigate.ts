// The one map from a subsystem `id` to the shell page that owns it (system-map scope) — so a status
// card can drill into the real view (the outbox grid, the extensions console, the DB browser) instead
// of being a dead end. One responsibility per file (FILE-LAYOUT): the id→surface lookup, nothing else.
//
// Only subsystems with a first-class page appear here. `gateway`/`bus`/`mcp` have no dedicated page
// (they are the transport/runtime itself), so they return null and stay non-clickable — honest, not a
// broken link. `store`/`ingest` → the data-console pages; `registry`/`extensions` → the Extensions
// console; `inbox`/`outbox` → their workflow pages.

import type { CoreSurface } from "@/features/shell";

const SUBSYSTEM_SURFACE: Record<string, CoreSurface> = {
  store: "data",
  ingest: "ingest",
  inbox: "inbox",
  outbox: "outbox",
  extensions: "extensions",
  registry: "extensions",
};

/** The shell surface that owns `subsystemId`, or `null` if it has no dedicated page. */
export function surfaceForSubsystem(subsystemId: string): CoreSurface | null {
  return SUBSYSTEM_SURFACE[subsystemId] ?? null;
}
