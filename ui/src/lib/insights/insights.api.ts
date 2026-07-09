// The insights api client — one call per export, mirroring the `insight.*` MCP verbs
// (insights umbrella scope + occurrences sub-scope). All calls ride the universal MCP bridge
// (`POST /mcp/call`) so the full nested `ListQuery` (cursor / tags / range) round-trips as JSON —
// `serde_urlencoded` (axum's `Query`) can't express nested maps/structs, and "one contract" (the
// scope) is the MCP bridge. The gateway re-checks `mcp:insight.<verb>:call` + the workspace wall.

import { invoke } from "@/lib/ipc/invoke";
import type {
  Insight,
  ListPage,
  ListQuery,
  OccCursor,
  OccurrencePage,
} from "./insights.types";

/** Drive an `insight.*` verb over the MCP bridge (the shell holds the token; the host gates). */
function mcp<T>(tool: string, args: Record<string, unknown>): Promise<T> {
  return invoke<T>("mcp_call", { tool, args });
}

/** List insights newest-first, keyset-paged. Mirrors `insight.list`. */
export function listInsights(query: ListQuery): Promise<ListPage> {
  return mcp<ListPage>("insight.list", { ...query });
}

/** Read one insight by id. Mirrors `insight.get`. */
export function getInsight(id: string): Promise<Insight | null> {
  return mcp<Insight | null>("insight.get", { id });
}

/** Ack an insight (open → acked). Mirrors `insight.ack` — the client passes the logical `ts`. */
export function ackInsight(id: string): Promise<void> {
  return mcp<void>("insight.ack", { id, ts: Date.now() });
}

/** Resolve an insight (* → resolved). Mirrors `insight.resolve`. */
export function resolveInsight(id: string, note?: string): Promise<void> {
  return mcp<void>("insight.resolve", { id, note, ts: Date.now() });
}

/** Hard-delete an insight, cascading its occurrence ring. Mirrors `insight.delete` (idempotent). */
export function deleteInsight(id: string): Promise<void> {
  return mcp<void>("insight.delete", { id });
}

/** Delete one occurrence (transaction) from an insight's ring by its `oseq`. Mirrors
 *  `insight.occurrence.delete` (idempotent). */
export function deleteOccurrence(insightId: string, oseq: number): Promise<void> {
  return mcp<void>("insight.occurrence.delete", {
    insight_id: insightId,
    oseq,
  });
}

/** Read the per-insight occurrence ring, newest-first. Mirrors `insight.occurrences`. */
export function listOccurrences(
  insightId: string,
  cursor?: OccCursor,
  limit?: number,
): Promise<OccurrencePage> {
  return mcp<OccurrencePage>("insight.occurrences", {
    insight_id: insightId,
    cursor,
    limit: limit ?? 50,
  });
}
