// The read-only SQL query client — the `store.query` verb (widget-builder Slice A, the "direct
// SurrealDB" source). Rides the host-mediated `POST /mcp/call` bridge (the `mcp_call` invoke verb),
// capability-gated server-side (`mcp:store.query:call`); the workspace comes from the session token (the
// hard wall, §7), never an argument; the SQL can never name a namespace. `store.query` is
// parse-allowlisted to a single SELECT + bounded (10k rows / 5s) at the host — the builder (Slice C) is
// convenience above that boundary, never instead of it.
//
// The store-SCHEMA reader (`readSchema`/`Schema`/`SchemaTable`/`SchemaColumn`) moved to the shared
// `@/lib/schema` module (it is a generic store concern, consumed by both the SQL builder and the rules
// data explorer — rules-editor-ux scope). Re-exported here for the existing SQL-builder import sites.

import { invoke } from "@/lib/ipc/invoke";

export { readSchema } from "@/lib/schema";
export type { Schema, SchemaTable, SchemaColumn } from "@/lib/schema";

/** A `store.query` result — column names + rows (JSON objects keyed by column). The table/chart
 *  views render `rows` directly; `columns` drives the header + the chart x/y picker. */
export interface QueryResult {
  columns: string[];
  rows: Record<string, unknown>[];
}

/** Run a read-only SurrealQL `sql` (with optional `$`-bound `vars`). Mirrors `store.query`. A write/
 *  multi/namespace statement is refused server-side at parse (rejected as a bad-input error), never
 *  run — Builder mode only ever generates a SELECT, but Code mode is gated the same way. */
export function runQuery(
  sql: string,
  vars?: Record<string, unknown>,
): Promise<QueryResult> {
  return invoke<QueryResult>("mcp_call", {
    tool: "store.query",
    args: vars ? { sql, vars } : { sql },
  });
}
