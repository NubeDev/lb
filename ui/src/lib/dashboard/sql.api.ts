// The read-only SQL API client — the `store.query` / `store.schema` verbs (widget-builder Slice A, the
// "direct SurrealDB" source). Like every other widget data path these ride the host-mediated
// `POST /mcp/call` bridge (the `mcp_call` invoke verb) — the builder consumes tools, it gets no bespoke
// REST surface. Both are capability-gated server-side (`mcp:store.query|schema:call`); the workspace
// comes from the session token (the hard wall, §7), never an argument; the SQL can never name a
// namespace. `store.query` is parse-allowlisted to a single SELECT + bounded (10k rows / 5s) at the
// host — the builder (Slice C) is convenience above that boundary, never instead of it.

import { invoke } from "@/lib/ipc/invoke";

/** A `store.query` result — column names + rows (JSON objects keyed by column). The table/chart
 *  views render `rows` directly; `columns` drives the header + the chart x/y picker. */
export interface QueryResult {
  columns: string[];
  rows: Record<string, unknown>[];
}

/** One column of a table as `store.schema` reports it. */
export interface SchemaColumn {
  name: string;
  type: string;
}

/** One table in the workspace + the columns the visual builder offers for it. */
export interface SchemaTable {
  name: string;
  columns: SchemaColumn[];
}

/** The workspace schema (every table + its columns) — the visual SQL builder's dropdown source. */
export interface Schema {
  tables: SchemaTable[];
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

/** Read the workspace's schema (tables + columns) for the visual builder. Mirrors `store.schema`. */
export function readSchema(): Promise<Schema> {
  return invoke<Schema>("mcp_call", { tool: "store.schema", args: {} });
}
